'use client';

import React, { useState } from 'react';
import { ChevronDown, ChevronRight, Code } from 'lucide-react';

const API_METHODS = [
  {
    category: 'Navigation',
    methods: [
      { name: 'page.goto(url)', desc: 'Navigate to URL' },
      { name: 'page.reload()', desc: 'Reload the page' },
      { name: 'page.goBack()', desc: 'Go back in history' },
      { name: 'page.goForward()', desc: 'Go forward in history' },
    ]
  },
  {
    category: 'Selectors & Waiting',
    methods: [
      { name: 'page.waitForSelector(selector)', desc: 'Wait for element to appear' },
      { name: 'page.waitForURL(pattern)', desc: 'Wait for URL to match pattern' },
      { name: 'page.waitForTimeout(ms)', desc: 'Wait for specified time' },
    ]
  },
  {
    category: 'Interaction',
    methods: [
      { name: 'page.click(selector)', desc: 'Click an element' },
      { name: 'page.fill(selector, value)', desc: 'Fill input with value' },
      { name: 'page.type(selector, text)', desc: 'Type text character by character' },
      { name: 'page.press(selector, key)', desc: 'Press a keyboard key' },
      { name: 'page.selectOption(selector, value)', desc: 'Select dropdown option' },
      { name: 'page.check(selector)', desc: 'Check a checkbox' },
      { name: 'page.uncheck(selector)', desc: 'Uncheck a checkbox' },
      { name: 'page.hover(selector)', desc: 'Hover over element' },
    ]
  },
  {
    category: 'Data Extraction',
    methods: [
      { name: 'page.title()', desc: 'Get page title' },
      { name: 'page.url()', desc: 'Get current URL' },
      { name: 'page.content()', desc: 'Get full HTML content' },
      { name: 'page.evaluate(fn)', desc: 'Run JavaScript in page context' },
      { name: 'page.scrapeText(selector)', desc: 'Extract text from element' },
      { name: 'page.scrapeAttribute(selector, attr)', desc: 'Extract attribute value' },
      { name: 'page.scrapeAll(selector, opts)', desc: 'Extract multiple elements' },
    ]
  },
  {
    category: 'Network Capture',
    methods: [
      { name: 'page.captureNetwork(key, pattern)', desc: 'Capture requests matching URL pattern' },
      { name: 'page.getCapturedResponse(key)', desc: 'Get captured response data' },
      { name: 'page.waitForNetworkCapture(key, timeout)', desc: 'Wait for capture to complete' },
    ]
  },
  {
    category: 'Automation Data',
    methods: [
      { name: 'page.setData(key, value)', desc: 'Store data for later retrieval' },
      { name: 'page.getData()', desc: 'Get all stored data' },
      { name: 'page.clearData()', desc: 'Clear all stored data' },
    ]
  },
  {
    category: 'User Interaction',
    methods: [
      { name: 'page.promptUser(message, opts)', desc: 'Pause for user input with optional auto-continue' },
    ]
  },
];

export function ApiReference() {
  const [isExpanded, setIsExpanded] = useState(false);
  const [expandedCategories, setExpandedCategories] = useState<Set<string>>(new Set());

  const toggleCategory = (category: string) => {
    setExpandedCategories(prev => {
      const next = new Set(prev);
      if (next.has(category)) {
        next.delete(category);
      } else {
        next.add(category);
      }
      return next;
    });
  };

  return (
    <div className="border border-border rounded-lg overflow-hidden bg-surface mt-3">
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full flex items-center justify-between px-3 py-2 bg-background hover:bg-surface transition-colors"
      >
        <div className="flex items-center gap-2 text-sm font-medium text-text-secondary">
          <Code className="w-4 h-4" />
          API Reference
        </div>
        {isExpanded ? (
          <ChevronDown className="w-4 h-4 text-text-tertiary" />
        ) : (
          <ChevronRight className="w-4 h-4 text-text-tertiary" />
        )}
      </button>

      {isExpanded && (
        <div className="border-t border-border max-h-[300px] overflow-y-auto custom-scrollbar">
          {API_METHODS.map((group) => (
            <div key={group.category} className="border-b border-border/50 last:border-b-0">
              <button
                onClick={() => toggleCategory(group.category)}
                className="w-full flex items-center gap-2 px-3 py-2 hover:bg-background transition-colors text-left"
              >
                {expandedCategories.has(group.category) ? (
                  <ChevronDown className="w-3 h-3 text-text-tertiary shrink-0" />
                ) : (
                  <ChevronRight className="w-3 h-3 text-text-tertiary shrink-0" />
                )}
                <span className="text-xs font-medium text-accent">{group.category}</span>
                <span className="text-[10px] text-text-tertiary">({group.methods.length})</span>
              </button>

              {expandedCategories.has(group.category) && (
                <div className="pb-2">
                  {group.methods.map((method) => (
                    <div key={method.name} className="px-6 py-1.5 hover:bg-background/50">
                      <code className="text-xs font-mono text-foreground">{method.name}</code>
                      <p className="text-[10px] text-text-tertiary mt-0.5">{method.desc}</p>
                    </div>
                  ))}
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
