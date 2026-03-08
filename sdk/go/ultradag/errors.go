package ultradag

import "fmt"

// APIError represents a non-200 HTTP response from the UltraDAG node.
type APIError struct {
	// StatusCode is the HTTP status code returned by the server.
	StatusCode int
	// Status is the HTTP status text (e.g. "404 Not Found").
	Status string
	// Body is the raw response body, useful for debugging.
	Body string
}

// Error implements the error interface.
func (e *APIError) Error() string {
	if e.Body != "" {
		return fmt.Sprintf("ultradag: API error %d %s: %s", e.StatusCode, e.Status, e.Body)
	}
	return fmt.Sprintf("ultradag: API error %d %s", e.StatusCode, e.Status)
}

// IsNotFound returns true if the error is a 404 response.
func (e *APIError) IsNotFound() bool {
	return e.StatusCode == 404
}

// IsServerError returns true if the error is a 5xx response.
func (e *APIError) IsServerError() bool {
	return e.StatusCode >= 500 && e.StatusCode < 600
}
