package main

import (
	"crypto/rand"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"path/filepath"
	"sync"
	"time"

	"github.com/nikhilramakrishnan/yarp/internal/termio"
)

const (
	defaultMaxInFlight = uint64(8)
	defaultStopTimeout = 5 * time.Second
	readBufferSize     = 64 * 1024
)

type ManagedSessionInfo struct {
	ID    string `json:"id"`
	Shell string `json:"shell"`
	CWD   string `json:"cwd"`
}

type SessionDataEvent struct {
	SessionID string
	Sequence  uint64
	Data      []byte
}

type SessionExitEvent struct {
	SessionID         string
	FinalDataSequence uint64
	ExitCode          int
	Err               error
	Requested         bool
}

type SessionErrorEvent struct {
	SessionID string
	Operation string
	Err       error
}

type SessionEventCallbacks struct {
	OnData  func(SessionDataEvent)
	OnExit  func(SessionExitEvent)
	OnError func(SessionErrorEvent)
}

type ptyStarter func(termio.Command, int, int) (termio.Pty, error)

type SessionManagerOption func(*SessionManager)

func withPTYStarter(start ptyStarter) SessionManagerOption {
	return func(manager *SessionManager) { manager.startPTY = start }
}

func withMaxInFlight(max uint64) SessionManagerOption {
	return func(manager *SessionManager) {
		if max > 0 {
			manager.maxInFlight = max
		}
	}
}

func withStopTimeout(timeout time.Duration) SessionManagerOption {
	return func(manager *SessionManager) {
		if timeout > 0 {
			manager.stopTimeout = timeout
		}
	}
}

type SessionManager struct {
	mu          sync.RWMutex
	sessions    map[string]*managedSession
	callbacks   SessionEventCallbacks
	startPTY    ptyStarter
	maxInFlight uint64
	stopTimeout time.Duration
}

type managedSession struct {
	info ManagedSessionInfo
	pty  termio.Pty

	writeMu sync.Mutex

	mu            sync.Mutex
	window        *sync.Cond
	sentSequence  uint64
	ackedSequence uint64
	stopRequested bool
	exited        bool
	exit          SessionExitEvent
	done          chan struct{}
	readDone      chan struct{}
}

func NewSessionManager(callbacks SessionEventCallbacks, options ...SessionManagerOption) *SessionManager {
	manager := &SessionManager{
		sessions:    make(map[string]*managedSession),
		callbacks:   callbacks,
		startPTY:    termio.Start,
		maxInFlight: defaultMaxInFlight,
		stopTimeout: defaultStopTimeout,
	}
	for _, option := range options {
		option(manager)
	}
	return manager
}

func (manager *SessionManager) Start(command termio.Command, cols, rows int) (ManagedSessionInfo, error) {
	id, err := newOpaqueSessionID()
	if err != nil {
		return ManagedSessionInfo{}, fmt.Errorf("creating session ID: %w", err)
	}
	return manager.startWithID(id, command, cols, rows)
}

func (manager *SessionManager) startWithID(id string, command termio.Command, cols, rows int) (ManagedSessionInfo, error) {
	if id == "" {
		return ManagedSessionInfo{}, errors.New("terminal session ID is empty")
	}
	manager.mu.RLock()
	_, exists := manager.sessions[id]
	manager.mu.RUnlock()
	if exists {
		return ManagedSessionInfo{}, fmt.Errorf("terminal session %q already exists", id)
	}
	pty, err := manager.startPTY(command, cols, rows)
	if err != nil {
		return ManagedSessionInfo{}, err
	}
	session := &managedSession{
		info: ManagedSessionInfo{
			ID:    id,
			Shell: filepath.Base(command.Path),
			CWD:   command.Dir,
		},
		pty:      pty,
		done:     make(chan struct{}),
		readDone: make(chan struct{}),
	}
	session.window = sync.NewCond(&session.mu)
	manager.mu.Lock()
	if _, exists := manager.sessions[id]; exists {
		manager.mu.Unlock()
		_ = pty.Close()
		return ManagedSessionInfo{}, fmt.Errorf("terminal session %q already exists", id)
	}
	manager.sessions[id] = session
	manager.mu.Unlock()

	go manager.readLoop(session)
	go manager.waitLoop(session)
	return session.info, nil
}

func (manager *SessionManager) Write(sessionID string, data []byte) error {
	session, err := manager.session(sessionID)
	if err != nil {
		return err
	}
	session.writeMu.Lock()
	defer session.writeMu.Unlock()
	_, err = session.pty.Write(data)
	if err != nil {
		manager.emitError(sessionID, "write", err)
	}
	return err
}

func (manager *SessionManager) Resize(sessionID string, cols, rows int) error {
	session, err := manager.session(sessionID)
	if err != nil {
		return err
	}
	err = session.pty.Resize(cols, rows)
	if err != nil {
		manager.emitError(sessionID, "resize", err)
	}
	return err
}

func (manager *SessionManager) Ack(sessionID string, sequence uint64) error {
	session, err := manager.session(sessionID)
	if err != nil {
		return err
	}
	session.mu.Lock()
	sentSequence := session.sentSequence
	if sequence > sentSequence {
		session.mu.Unlock()
		return fmt.Errorf("acknowledging terminal output %d beyond sent sequence %d", sequence, sentSequence)
	}
	if sequence > session.ackedSequence {
		session.ackedSequence = sequence
		session.window.Broadcast()
	}
	remove := session.exited && session.ackedSequence >= session.sentSequence
	session.mu.Unlock()
	if remove {
		manager.removeSession(session)
	}
	return nil
}

// Stop asks the PTY to terminate and returns only after the child has been
// reaped. A timeout is reported to the caller, which must keep the pane
// visible rather than pretending that an unconfirmed process is gone.
func (manager *SessionManager) Stop(sessionID string) error {
	manager.mu.RLock()
	session := manager.sessions[sessionID]
	manager.mu.RUnlock()
	if session == nil {
		return nil
	}

	session.mu.Lock()
	session.stopRequested = true
	session.window.Broadcast()
	session.mu.Unlock()
	closeErr := session.pty.Close()

	select {
	case <-session.done:
		manager.removeSession(session)
		if closeErr != nil && !errors.Is(closeErr, io.EOF) {
			return closeErr
		}
		return nil
	case <-time.After(manager.stopTimeout):
		return fmt.Errorf("timed out waiting for terminal session %s to stop", sessionID)
	}
}

func (manager *SessionManager) StopAll() error {
	manager.mu.RLock()
	ids := make([]string, 0, len(manager.sessions))
	for id := range manager.sessions {
		ids = append(ids, id)
	}
	manager.mu.RUnlock()

	var wait sync.WaitGroup
	errs := make(chan error, len(ids))
	for _, id := range ids {
		wait.Add(1)
		go func(sessionID string) {
			defer wait.Done()
			if err := manager.Stop(sessionID); err != nil {
				errs <- err
			}
		}(id)
	}
	wait.Wait()
	close(errs)
	var combined error
	for err := range errs {
		combined = errors.Join(combined, err)
	}
	return combined
}

func (manager *SessionManager) Count() int {
	manager.mu.RLock()
	defer manager.mu.RUnlock()
	return len(manager.sessions)
}

func (manager *SessionManager) session(sessionID string) (*managedSession, error) {
	manager.mu.RLock()
	session := manager.sessions[sessionID]
	manager.mu.RUnlock()
	if session == nil {
		return nil, fmt.Errorf("terminal session %q is not running", sessionID)
	}
	return session, nil
}

func (manager *SessionManager) readLoop(session *managedSession) {
	defer close(session.readDone)
	buf := make([]byte, readBufferSize)
	for {
		n, err := session.pty.Read(buf)
		if n > 0 {
			data := append([]byte(nil), buf[:n]...)
			session.mu.Lock()
			for !session.stopRequested && session.sentSequence-session.ackedSequence >= manager.maxInFlight {
				session.window.Wait()
			}
			session.sentSequence++
			sequence := session.sentSequence
			session.mu.Unlock()
			if callback := manager.callbacks.OnData; callback != nil {
				callback(SessionDataEvent{SessionID: session.info.ID, Sequence: sequence, Data: data})
			}
		}
		if err != nil {
			session.mu.Lock()
			requested := session.stopRequested
			session.mu.Unlock()
			if !requested && !errors.Is(err, io.EOF) {
				manager.emitError(session.info.ID, "read", err)
			}
			return
		}
	}
}

func (manager *SessionManager) waitLoop(session *managedSession) {
	exitCode, waitErr := session.pty.Wait()
	<-session.readDone
	session.mu.Lock()
	exit := SessionExitEvent{
		SessionID:         session.info.ID,
		FinalDataSequence: session.sentSequence,
		ExitCode:          exitCode,
		Err:               waitErr,
		Requested:         session.stopRequested,
	}
	session.exit = exit
	session.mu.Unlock()

	session.mu.Lock()
	session.exited = true
	remove := session.ackedSequence >= session.sentSequence
	session.mu.Unlock()
	if callback := manager.callbacks.OnExit; callback != nil {
		callback(exit)
	}
	close(session.done)
	if remove {
		manager.removeSession(session)
	}
}

func (manager *SessionManager) emitError(sessionID, operation string, err error) {
	if callback := manager.callbacks.OnError; callback != nil {
		callback(SessionErrorEvent{SessionID: sessionID, Operation: operation, Err: err})
	}
}

func (manager *SessionManager) removeSession(session *managedSession) {
	manager.mu.Lock()
	if manager.sessions[session.info.ID] == session {
		delete(manager.sessions, session.info.ID)
	}
	manager.mu.Unlock()
}

func newOpaqueSessionID() (string, error) {
	raw := make([]byte, 16)
	if _, err := rand.Read(raw); err != nil {
		return "", err
	}
	return hex.EncodeToString(raw), nil
}
