package main

import (
	"crypto/tls"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
)

// Simple HTTP/HTTPS proxy server for testing GitHub CLI proxying
// This logs all requests and forwards them to the actual destination

type ProxyHandler struct{}

func (h *ProxyHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	// Log the request
	log.Printf("Proxying request: %s %s", r.Method, r.URL.String())
	log.Printf("  Host: %s", r.Host)
	log.Printf("  Headers: %v", r.Header)

	// For CONNECT method (HTTPS tunneling)
	if r.Method == http.MethodConnect {
		h.handleHTTPS(w, r)
		return
	}

	// For regular HTTP requests
	h.handleHTTP(w, r)
}

func (h *ProxyHandler) handleHTTP(w http.ResponseWriter, r *http.Request) {
	// Create a new request to the target
	client := &http.Client{
		CheckRedirect: func(req *http.Request, via []*http.Request) error {
			return http.ErrUseLastResponse
		},
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

func (h *ProxyHandler) handleHTTPS(w http.ResponseWriter, r *http.Request) {
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

	// Send 200 Connection Established to client
	w.WriteHeader(http.StatusOK)

	// Hijack the connection
	hijacker, ok := w.(http.Hijacker)
	if !ok {
		http.Error(w, "Hijacking not supported", http.StatusInternalServerError)
		return
	}

	clientConn, _, err := hijacker.Hijack()
	if err != nil {
		log.Printf("  Error hijacking connection: %v", err)
		return
	}
	defer clientConn.Close()

	// Relay data between client and destination
	go io.Copy(destConn, clientConn)
	io.Copy(clientConn, destConn)

	log.Printf("  HTTPS tunnel closed for %s", r.Host)
}

func main() {
	port := "8888"
	if len(os.Args) > 1 {
		port = os.Args[1]
	}

	handler := &ProxyHandler{}
	server := &http.Server{
		Addr:    ":" + port,
		Handler: handler,
	}

	log.Printf("Starting HTTP/HTTPS proxy server on port %s", port)
	log.Printf("Usage: export HTTPS_PROXY=http://localhost:%s", port)
	log.Printf("       gh repo view cli/cli")
	log.Fatal(server.ListenAndServe())
}
