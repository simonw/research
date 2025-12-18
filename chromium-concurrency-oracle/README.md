# Chromium Concurrency Capacity Oracle

Investigation of concurrent Playwright browser contexts in linuxserver/chromium containers with Xvfb and WebRTC streaming.

## Key Findings

### Capacity Estimates
- **Conservative**: 2-3 concurrent contexts (recommended for production)
- **Moderate**: 4-6 concurrent contexts (acceptable with monitoring)  
- **Aggressive**: 7-10 concurrent contexts (high risk)

### Resource Scaling
- **Memory**: 50-100MB per additional context
- **CPU**: 0.3-0.7 cores per additional context
- **Base overhead**: 400-800MB + 0.2-0.5 cores (idle Chrome + Selkies)

### Limiting Factors
1. **Memory exhaustion**: 2GB shm_size limit in containers
2. **Chrome architecture**: Multi-process renderer isolation
3. **Container constraints**: Static configuration, no dynamic scaling
4. **CDP connection limits**: Single WebSocket per browser instance

## Quick Test

```bash
# Start test environment
docker-compose -f docker-compose.test.yml up -d

# Wait for Chrome to be ready
sleep 60

# Run capacity test
docker-compose -f docker-compose.test.yml --profile test run --rm playwright-tester

# Check results
docker logs chromium-capacity-test
```

## Files

- `capacity-estimates.md` - Detailed capacity analysis and scaling models
- `measurement-plan.md` - Comprehensive testing methodology and scripts
- `test-script.js` - Quick capacity testing script
- `docker-compose.test.yml` - Test environment configuration
- `notes.md` - Investigation notes and research findings

## Production Recommendations

### Safe Configuration (3-5 contexts)
```yaml
services:
  chromium:
    image: lscr.io/linuxserver/chromium:latest
    environment:
      - CHROME_CLI=--remote-debugging-port=9222 --remote-debugging-address=0.0.0.0 --disable-gpu --no-sandbox
    shm_size: 2gb
    deploy:
      resources:
        limits:
          cpus: '6.0'
          memory: 8GB
        reservations:
          cpus: '2.0'
          memory: 4GB
```

### Monitoring
- Memory usage >70% of allocated = warning
- CPU utilization >70% sustained = warning  
- Context creation failures >5%/hour = warning
- Implement automated recovery and horizontal scaling

## Research Sources

Based on analysis of:
- Playwright CDP session management patterns
- Chromium multi-process architecture  
- LinuxServer container limitations
- Session multiplexing performance analysis
- Docker Chrome performance optimization research
