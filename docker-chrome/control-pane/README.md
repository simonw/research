# Docker Chrome Control Pane

A Next.js-based control panel for the remote Docker Chrome instance.

## Overview

This application provides a user interface to interact with a headless Chrome instance running on Cloud Run. It features:

- **Live Stream**: View the remote browser in a mobile-sized viewport (iPhone SE dimensions).
- **Network Inspector**: Real-time log of network requests captured via WebSocket.
- **Remote Control**: 
  - Navigate to URLs.
  - Inject JavaScript code immediately.
  - Manage persistent scripts that run on every page load.
- **Status Monitoring**: View CDP connection status and port information.

## Architecture

- **Framework**: Next.js 15 (App Router)
- **Styling**: Tailwind CSS 4
- **State Management**: React Hooks + WebSocket
- **Icons**: Lucide React

## Setup

1. Install dependencies:
   ```bash
   npm install
   ```

2. Run development server:
   ```bash
   npm run dev
   ```

3. Open [http://localhost:3000](http://localhost:3000)

## Configuration

The application is currently hardcoded to connect to:
- HTTP: `https://docker-chrome-432753364585.us-central1.run.app`
- WSS: `wss://docker-chrome-432753364585.us-central1.run.app/ws`

To change this, update `API_BASE` and `WS_URL` in `src/app/page.tsx` and `src/components/control-panel.tsx`.

## Project Structure

- `src/app`: App router pages and layouts.
- `src/components`: Reusable UI components (`BrowserFrame`, `NetworkPanel`, `ControlPanel`).
- `src/lib`: Type definitions.
