package backup

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"mime/multipart"
	"net/http"
	"net/textproto"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"time"
)

// Google Drive connector.
//
// yarp ships no credentials: the user creates an OAuth client in their own
// Google Cloud project (a "TV & limited input" client works well) and pastes
// its ID/secret into settings. Authorization uses the device flow — yarp
// prints a URL and code, the user approves on any browser — and the refresh
// token is stored locally in settings.json alongside everything else the
// user owns. Backups upload to a folder in the user's Drive; nothing else is
// read or written (scope: drive.file).

const (
	deviceCodeURL = "https://oauth2.googleapis.com/device/code"
	tokenURL      = "https://oauth2.googleapis.com/token"
	uploadURL     = "https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart"
	driveScope    = "https://www.googleapis.com/auth/drive.file"
)

// DriveAuth holds the user's own OAuth client and grant.
type DriveAuth struct {
	ClientID     string
	ClientSecret string
	RefreshToken string
}

// DeviceAuthPrompt is what the user must do to approve access.
type DeviceAuthPrompt struct {
	VerificationURL string
	UserCode        string
	deviceCode      string
	interval        time.Duration
	expiresAt       time.Time
}

// StartDeviceAuth begins the device flow and returns the prompt to show.
func StartDeviceAuth(ctx context.Context, clientID string) (*DeviceAuthPrompt, error) {
	form := url.Values{"client_id": {clientID}, "scope": {driveScope}}
	var resp struct {
		DeviceCode      string `json:"device_code"`
		UserCode        string `json:"user_code"`
		VerificationURL string `json:"verification_url"`
		ExpiresIn       int    `json:"expires_in"`
		Interval        int    `json:"interval"`
	}
	if err := postForm(ctx, deviceCodeURL, form, &resp); err != nil {
		return nil, fmt.Errorf("starting device authorization: %w", err)
	}
	interval := time.Duration(resp.Interval) * time.Second
	if interval <= 0 {
		interval = 5 * time.Second
	}
	return &DeviceAuthPrompt{
		VerificationURL: resp.VerificationURL,
		UserCode:        resp.UserCode,
		deviceCode:      resp.DeviceCode,
		interval:        interval,
		expiresAt:       time.Now().Add(time.Duration(resp.ExpiresIn) * time.Second),
	}, nil
}

// Wait polls until the user approves and returns the refresh token.
func (p *DeviceAuthPrompt) Wait(ctx context.Context, clientID, clientSecret string) (string, error) {
	for time.Now().Before(p.expiresAt) {
		select {
		case <-ctx.Done():
			return "", ctx.Err()
		case <-time.After(p.interval):
		}
		form := url.Values{
			"client_id":     {clientID},
			"client_secret": {clientSecret},
			"device_code":   {p.deviceCode},
			"grant_type":    {"urn:ietf:params:oauth:grant-type:device_code"},
		}
		var resp struct {
			RefreshToken string `json:"refresh_token"`
			Error        string `json:"error"`
		}
		if err := postForm(ctx, tokenURL, form, &resp); err != nil {
			return "", err
		}
		switch resp.Error {
		case "":
			if resp.RefreshToken == "" {
				return "", fmt.Errorf("authorization response had no refresh token")
			}
			return resp.RefreshToken, nil
		case "authorization_pending":
			continue
		case "slow_down":
			p.interval += 2 * time.Second
		default:
			return "", fmt.Errorf("authorization failed: %s", resp.Error)
		}
	}
	return "", fmt.Errorf("device authorization expired before approval")
}

// accessToken exchanges the refresh token.
func (a DriveAuth) accessToken(ctx context.Context) (string, error) {
	form := url.Values{
		"client_id":     {a.ClientID},
		"client_secret": {a.ClientSecret},
		"refresh_token": {a.RefreshToken},
		"grant_type":    {"refresh_token"},
	}
	var resp struct {
		AccessToken string `json:"access_token"`
		Error       string `json:"error"`
	}
	if err := postForm(ctx, tokenURL, form, &resp); err != nil {
		return "", err
	}
	if resp.AccessToken == "" {
		return "", fmt.Errorf("token refresh failed: %s (re-run `yarp backup gdrive-auth`)", resp.Error)
	}
	return resp.AccessToken, nil
}

// UploadToDrive sends the archive to the user's Drive, optionally into
// folderID, and returns the created file ID.
func UploadToDrive(ctx context.Context, auth DriveAuth, archive, folderID string) (string, error) {
	token, err := auth.accessToken(ctx)
	if err != nil {
		return "", err
	}
	f, err := os.Open(archive)
	if err != nil {
		return "", err
	}
	defer f.Close()

	meta := map[string]any{"name": filepath.Base(archive)}
	if folderID != "" {
		meta["parents"] = []string{folderID}
	}
	metaRaw, err := json.Marshal(meta)
	if err != nil {
		return "", err
	}

	var body bytes.Buffer
	mw := multipart.NewWriter(&body)
	metaHdr := textproto.MIMEHeader{"Content-Type": {"application/json; charset=UTF-8"}}
	part, err := mw.CreatePart(metaHdr)
	if err != nil {
		return "", err
	}
	if _, err := part.Write(metaRaw); err != nil {
		return "", err
	}
	fileHdr := textproto.MIMEHeader{"Content-Type": {"application/gzip"}}
	part, err = mw.CreatePart(fileHdr)
	if err != nil {
		return "", err
	}
	if _, err := io.Copy(part, f); err != nil {
		return "", err
	}
	if err := mw.Close(); err != nil {
		return "", err
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodPost, uploadURL, &body)
	if err != nil {
		return "", err
	}
	req.Header.Set("Authorization", "Bearer "+token)
	req.Header.Set("Content-Type", "multipart/related; boundary="+mw.Boundary())
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()
	raw, _ := io.ReadAll(io.LimitReader(resp.Body, 1<<20))
	if resp.StatusCode != http.StatusOK {
		return "", fmt.Errorf("drive upload failed: %s: %s", resp.Status, strings.TrimSpace(string(raw)))
	}
	var created struct {
		ID string `json:"id"`
	}
	if err := json.Unmarshal(raw, &created); err != nil {
		return "", err
	}
	return created.ID, nil
}

func postForm(ctx context.Context, endpoint string, form url.Values, out any) error {
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, endpoint, strings.NewReader(form.Encode()))
	if err != nil {
		return err
	}
	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	raw, err := io.ReadAll(io.LimitReader(resp.Body, 1<<20))
	if err != nil {
		return err
	}
	// OAuth endpoints return errors as JSON bodies with non-200 statuses;
	// decode either way and let callers inspect the Error field.
	return json.Unmarshal(raw, out)
}
