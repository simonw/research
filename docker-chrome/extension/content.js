(function() {
    if (window.__CLOUD_BROWSER_INJECTED) return;
    window.__CLOUD_BROWSER_INJECTED = true;

    console.log('[Cloud Browser] Hook injected');

    window.addEventListener('message', (event) => {
        if (event.source !== window) return;
        if (event.data?.type === 'CLOUD_BROWSER_EXEC') {
            try {
                const result = eval(event.data.code);
                window.postMessage({
                    type: 'CLOUD_BROWSER_RESULT',
                    id: event.data.id,
                    result
                }, '*');
            } catch (error) {
                window.postMessage({
                    type: 'CLOUD_BROWSER_ERROR',
                    id: event.data.id,
                    error: error.message
                }, '*');
            }
        }
    });

    const originalFetch = window.fetch;
    window.fetch = async function(...args) {
        return originalFetch.apply(this, args);
    };

})();
