// Simple App JavaScript
(function() {
    'use strict';

    // Update the message
    const messageEl = document.getElementById('message');
    messageEl.textContent = 'JavaScript is working! Click the button below.';

    // Counter functionality
    let count = 0;
    const counterBtn = document.getElementById('counter-btn');

    counterBtn.addEventListener('click', function() {
        count++;
        counterBtn.textContent = 'Count: ' + count;

        // Add a fun effect
        counterBtn.style.transform = 'scale(1.1)';
        setTimeout(function() {
            counterBtn.style.transform = '';
        }, 100);
    });

    // Log that the app loaded
    console.log('Simple App loaded successfully!');
})();
