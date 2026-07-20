package main

import (
	"bytes"
	"errors"
	"io"
	"sync"
	"testing"
	"time"

	"github.com/nikhilramakrishnan/yarp/internal/termio"
)

type fakeRead struct {
	data []byte
	err  error
}

type fakeExit struct {
	code int
	err  error
}

type fakePTY struct {
	reads chan fakeRead
	wait  chan fakeExit

	mu         sync.Mutex
	writes     bytes.Buffer
	cols       int
	rows       int
	closeStops bool
	finishOnce sync.Once
}

func newFakePTY(closeStops bool) *fakePTY {
	return &fakePTY{
		reads:      make(chan fakeRead, 32),
		wait:       make(chan fakeExit, 1),
		closeStops: closeStops,
	}
}

func (pty *fakePTY) Read(buffer []byte) (int, error) {
	result := <-pty.reads
	return copy(buffer, result.data), result.err
}

func (pty *fakePTY) Write(data []byte) (int, error) {
	pty.mu.Lock()
	defer pty.mu.Unlock()
	return pty.writes.Write(data)
}

func (pty *fakePTY) Resize(cols, rows int) error {
	pty.mu.Lock()
	pty.cols, pty.rows = cols, rows
	pty.mu.Unlock()
	return nil
}

func (pty *fakePTY) Wait() (int, error) {
	result := <-pty.wait
	return result.code, result.err
}

func (pty *fakePTY) Close() error {
	if pty.closeStops {
		pty.finish(0, nil)
	}
	return nil
}

func (pty *fakePTY) emit(data string) {
	pty.reads <- fakeRead{data: []byte(data)}
}

func (pty *fakePTY) finish(code int, err error) {
	pty.finishOnce.Do(func() {
		pty.reads <- fakeRead{err: io.EOF}
		pty.wait <- fakeExit{code: code, err: err}
	})
}

func (pty *fakePTY) written() string {
	pty.mu.Lock()
	defer pty.mu.Unlock()
	return pty.writes.String()
}

func (pty *fakePTY) size() (int, int) {
	pty.mu.Lock()
	defer pty.mu.Unlock()
	return pty.cols, pty.rows
}

func fakeStarter(created chan<- *fakePTY, closeStops bool) ptyStarter {
	return func(termio.Command, int, int) (termio.Pty, error) {
		pty := newFakePTY(closeStops)
		created <- pty
		return pty, nil
	}
}

func TestSessionManagerKeepsSessionsIndependent(t *testing.T) {
	created := make(chan *fakePTY, 2)
	dataEvents := make(chan SessionDataEvent, 4)
	exitEvents := make(chan SessionExitEvent, 2)
	manager := NewSessionManager(SessionEventCallbacks{
		OnData: func(event SessionDataEvent) { dataEvents <- event },
		OnExit: func(event SessionExitEvent) { exitEvents <- event },
	}, withPTYStarter(fakeStarter(created, true)))

	first, err := manager.Start(termio.Command{Path: "/bin/sh", Dir: "/tmp/first"}, 80, 24)
	if err != nil {
		t.Fatal(err)
	}
	firstPTY := <-created
	second, err := manager.Start(termio.Command{Path: "/bin/zsh", Dir: "/tmp/second"}, 90, 30)
	if err != nil {
		t.Fatal(err)
	}
	secondPTY := <-created
	if first.ID == second.ID || first.CWD == second.CWD || manager.Count() != 2 {
		t.Fatalf("sessions are not independent: first=%+v second=%+v count=%d", first, second, manager.Count())
	}

	firstPTY.emit("first-output")
	secondPTY.emit("second-output")
	seen := make(map[string]SessionDataEvent)
	for range 2 {
		event := <-dataEvents
		seen[event.SessionID] = event
	}
	if string(seen[first.ID].Data) != "first-output" || string(seen[second.ID].Data) != "second-output" {
		t.Fatalf("misrouted data: %+v", seen)
	}
	if err := manager.Ack(first.ID, seen[first.ID].Sequence); err != nil {
		t.Fatal(err)
	}
	if err := manager.Ack(second.ID, seen[second.ID].Sequence); err != nil {
		t.Fatal(err)
	}
	if err := manager.Write(first.ID, []byte("alpha")); err != nil {
		t.Fatal(err)
	}
	if err := manager.Write(second.ID, []byte("bravo")); err != nil {
		t.Fatal(err)
	}
	if firstPTY.written() != "alpha" || secondPTY.written() != "bravo" {
		t.Fatalf("misrouted input: first=%q second=%q", firstPTY.written(), secondPTY.written())
	}
	if err := manager.Resize(second.ID, 120, 44); err != nil {
		t.Fatal(err)
	}
	if cols, rows := secondPTY.size(); cols != 120 || rows != 44 {
		t.Fatalf("resize = %dx%d, want 120x44", cols, rows)
	}

	firstPTY.finish(0, nil)
	secondPTY.finish(7, nil)
	exits := map[string]SessionExitEvent{}
	for range 2 {
		event := <-exitEvents
		exits[event.SessionID] = event
	}
	if exits[first.ID].ExitCode != 0 || exits[second.ID].ExitCode != 7 {
		t.Fatalf("wrong exits: %+v", exits)
	}
	waitForCount(t, manager, 0)
}

func TestSessionManagerBackpressuresUntilContiguousAck(t *testing.T) {
	created := make(chan *fakePTY, 1)
	dataEvents := make(chan SessionDataEvent, 4)
	manager := NewSessionManager(SessionEventCallbacks{
		OnData: func(event SessionDataEvent) { dataEvents <- event },
	}, withPTYStarter(fakeStarter(created, true)), withMaxInFlight(2))
	info, err := manager.Start(termio.Command{Path: "/bin/sh"}, 80, 24)
	if err != nil {
		t.Fatal(err)
	}
	pty := <-created
	pty.emit("one")
	pty.emit("two")
	pty.emit("three")
	first := <-dataEvents
	second := <-dataEvents
	if first.Sequence != 1 || second.Sequence != 2 {
		t.Fatalf("sequences = %d, %d", first.Sequence, second.Sequence)
	}
	select {
	case event := <-dataEvents:
		t.Fatalf("third batch escaped backpressure: %+v", event)
	case <-time.After(40 * time.Millisecond):
	}
	if err := manager.Ack(info.ID, 1); err != nil {
		t.Fatal(err)
	}
	select {
	case event := <-dataEvents:
		if event.Sequence != 3 || string(event.Data) != "three" {
			t.Fatalf("third event = %+v", event)
		}
	case <-time.After(time.Second):
		t.Fatal("third batch did not resume after acknowledgement")
	}
	if err := manager.Ack(info.ID, 3); err != nil {
		t.Fatal(err)
	}
	pty.finish(0, nil)
}

func TestSessionManagerExitReportsFinalDataAndAcceptsLateAck(t *testing.T) {
	created := make(chan *fakePTY, 1)
	dataEvents := make(chan SessionDataEvent, 2)
	exitEvents := make(chan SessionExitEvent, 1)
	manager := NewSessionManager(SessionEventCallbacks{
		OnData: func(event SessionDataEvent) { dataEvents <- event },
		OnExit: func(event SessionExitEvent) { exitEvents <- event },
	}, withPTYStarter(fakeStarter(created, true)))
	info, err := manager.Start(termio.Command{Path: "/bin/sh"}, 80, 24)
	if err != nil {
		t.Fatal(err)
	}
	pty := <-created
	pty.emit("before-exit")
	pty.emit("last-byte")
	pty.finish(3, nil)
	<-dataEvents
	<-dataEvents
	exit := <-exitEvents
	if exit.FinalDataSequence != 2 || exit.ExitCode != 3 {
		t.Fatalf("exit = %+v", exit)
	}
	if manager.Count() != 1 {
		t.Fatalf("session removed before final renderer ack; count=%d", manager.Count())
	}
	if err := manager.Ack(info.ID, exit.FinalDataSequence); err != nil {
		t.Fatalf("late final ack failed: %v", err)
	}
	waitForCount(t, manager, 0)
}

func TestSessionManagerStopDoesNotHideUnconfirmedProcess(t *testing.T) {
	created := make(chan *fakePTY, 1)
	exitEvents := make(chan SessionExitEvent, 1)
	manager := NewSessionManager(SessionEventCallbacks{
		OnExit: func(event SessionExitEvent) { exitEvents <- event },
	}, withPTYStarter(fakeStarter(created, false)), withStopTimeout(30*time.Millisecond))
	info, err := manager.Start(termio.Command{Path: "/bin/sh"}, 80, 24)
	if err != nil {
		t.Fatal(err)
	}
	pty := <-created
	if err := manager.Stop(info.ID); err == nil {
		t.Fatal("Stop succeeded without the child exiting")
	}
	if manager.Count() != 1 {
		t.Fatalf("unconfirmed process disappeared from manager; count=%d", manager.Count())
	}
	pty.finish(0, nil)
	exit := <-exitEvents
	if !exit.Requested {
		t.Fatalf("exit was not marked requested: %+v", exit)
	}
	waitForCount(t, manager, 0)
}

func TestSessionManagerRejectsAckBeyondSentOutput(t *testing.T) {
	created := make(chan *fakePTY, 1)
	manager := NewSessionManager(SessionEventCallbacks{}, withPTYStarter(fakeStarter(created, true)))
	info, err := manager.Start(termio.Command{Path: "/bin/sh"}, 80, 24)
	if err != nil {
		t.Fatal(err)
	}
	pty := <-created
	if err := manager.Ack(info.ID, 1); err == nil {
		t.Fatal("ack beyond sent sequence succeeded")
	}
	pty.finish(0, errors.New("test exit"))
}

func waitForCount(t *testing.T, manager *SessionManager, want int) {
	t.Helper()
	deadline := time.Now().Add(time.Second)
	for time.Now().Before(deadline) {
		if manager.Count() == want {
			return
		}
		time.Sleep(time.Millisecond)
	}
	t.Fatalf("session count = %d, want %d", manager.Count(), want)
}
