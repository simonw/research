package main

import (
	"crypto/tls"
	"fmt"
	"io"
	"log"
	"net"
	"net/http"
	"os"
)

// Unix Socket HTTP Proxy
// This creates a Unix domain socket that proxies HTTP/HTTPS requests
// Usage with gh: gh config set http_unix_socket /tmp/gh-proxy.sock

type UnixProxyHandler struct{}

func (h *UnixProxyHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	// Log the request
	log.Printf("Unix socket proxying: %s %s", r.Method, r.URL.String())
	log.Printf("  Host: %s", r.Host)

	// For CONNECT method (HTTPS tunneling)
	if r.Method == http.MethodConnect {
		h.handleHTTPS(w, r)
		return
	}

	// For regular HTTP requests
	h.handleHTTP(w, r)
}

func (h *UnixProxyHandler) handleHTTP(w http.ResponseWriter, r *http.Request) {
	// Create a new request to the target
	client := &http.Client{}

	// If the URL doesn't have a scheme, it's a relative URL
	// We need to reconstruct the full URL
	if r.URL.Scheme == "" {
		r.URL.Scheme = "https"
		r.URL.Host = r.Host
	}

	// Copy the original request
	outReq, err := http.NewRequest(r.Method, r.URL.String(), r.Body)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	// Copy headers
	for key, values := range r.Header {
		for _, value := range values {
			outReq.Header.Add(key, value)
		}
	}

	// Make the request
	resp, err := client.Do(outReq)
	if err != nil {
		log.Printf("  Error: %v", err)
		http.Error(w, err.Error(), http.StatusBadGateway)
		return
	}
	defer resp.Body.Close()

	log.Printf("  Response: %d %s", resp.StatusCode, resp.Status)

	// Copy response headers
	for key, values := range resp.Header {
		for _, value := range values {
			w.Header().Add(key, value)
		}
	}

	// Write status code
	w.WriteHeader(resp.StatusCode)

	// Copy response body
	io.Copy(w, resp.Body)
}

func (h *UnixProxyHandler) handleHTTPS(w http.ResponseWriter, r *http.Request) {
	log.Printf("  HTTPS CONNECT to %s", r.Host)

	// Establish connection to destination
	destConn, err := tls.Dial("tcp", r.Host, &tls.Config{
		InsecureSkipVerify: false,
	})
	if err != nil {
		log.Printf("  Error connecting to %s: %v", r.Host, err)
		http.Error(w, err.Error(), http.StatusBadGateway)
		return
	}
	defer destConn.Close()

	// Hijack the connection
	hijacker, ok := w.(http.Hijacker)
	if !ok {
		http.Error(w, "Hijacking not supported", http.StatusInternalServerError)
		return
	}

	clientConn, bufrw, err := hijacker.Hijack()
	if err != nil {
		log.Printf("  Error hijacking connection: %v", err)
		return
	}
	defer clientConn.Close()

	// Send 200 Connection Established to client
	bufrw.WriteString("HTTP/1.1 200 Connection Established\r\n\r\n")
	bufrw.Flush()

	// Relay data between client and destination
	go io.Copy(destConn, clientConn)
	io.Copy(clientConn, destConn)

	log.Printf("  HTTPS tunnel closed for %s", r.Host)
}

func main() {
	socketPath := "/tmp/gh-proxy.sock"
	if len(os.Args) > 1 {
		socketPath = os.Args[1]
	}

	// Remove existing socket if it exists
	os.Remove(socketPath)

	// Create Unix domain socket listener
	listener, err := net.Listen("unix", socketPath)
	if err != nil {
		log.Fatalf("Failed to create Unix socket: %v", err)
	}
	defer listener.Close()
	defer os.Remove(socketPath)

	// Make socket accessible
	os.Chmod(socketPath, 0777)

	handler := &UnixProxyHandler{}
	server := &http.Server{
		Handler: handler,
	}

	log.Printf("Starting Unix socket proxy at %s", socketPath)
	log.Printf("Usage: gh config set http_unix_socket %s", socketPath)
	log.Printf("       gh repo view cli/cli")
	log.Fatal(server.Serve(listener))
}
