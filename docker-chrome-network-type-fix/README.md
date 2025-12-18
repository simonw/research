# Docker Chrome Network Type Field Investigation

## Problem
The 'type' field in NetworkRequest objects is empty because the CDP event handler is not extracting the resource type from Network.requestWillBeSent events.

## Root Cause
The current implementation extracts `event.request.url` and `event.request.method` but misses `event.type`, which contains the resource type (Document, Stylesheet, Image, etc.).

## Solution
Add `type: event.type` to the NETWORK_REQUEST broadcast in the CDP event handler.

## Files to Modify
- `docker-chrome/server/index.js`: Add type field extraction in Network.requestWillBeSent handler

## Testing
After fix, network requests should show proper resource types instead of empty strings.
