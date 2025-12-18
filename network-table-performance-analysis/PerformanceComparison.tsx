// PerformanceComparison.tsx - Before/After comparison
import React, { useState, useMemo } from 'react';
import { NetworkEventTable } from './NetworkEventTable';

// Generate sample network events
function generateSampleEvents(count: number) {
  const methods = ['GET', 'POST', 'PUT', 'DELETE'];
  const statuses = [200, 201, 302, 404, 500];
  const types = ['Document', 'XHR', 'Script', 'Image', 'Stylesheet'];
  
  return Array.from({ length: count }, (_, i) => ({
    id: `event-${i}`,
    url: `https://api.example.com/endpoint/${i}`,
    method: methods[i % methods.length],
    status: statuses[i % statuses.length],
    size: Math.floor(Math.random() * 100000),
    timestamp: Date.now() - (count - i) * 1000,
    type: types[i % types.length]
  }));
}

// Traditional table implementation (slow)
function TraditionalTable({ events }: { events: any[] }) {
  return (
    <div className="border rounded max-h-96 overflow-auto">
      <div className="flex border-b bg-gray-50 font-medium text-xs sticky top-0">
        <div className="w-20 p-2">Time</div>
        <div className="w-12 p-2">Method</div>
        <div className="flex-1 p-2">URL</div>
        <div className="w-16 p-2">Status</div>
        <div className="w-20 p-2">Size</div>
        <div className="w-16 p-2">Type</div>
      </div>
      {events.map((event, index) => (
        <div key={event.id} className="flex border-b border-gray-200">
          <div className="w-20 p-2 text-xs text-gray-500">
            {new Date(event.timestamp).toLocaleTimeString()}
          </div>
          <div className="w-12 p-2 text-xs font-mono">{event.method}</div>
          <div className="flex-1 p-2 text-sm truncate">{event.url}</div>
          <div className="w-16 p-2 text-xs">{event.status}</div>
          <div className="w-20 p-2 text-xs text-gray-500">
            {event.size > 1024 ? `${(event.size / 1024).toFixed(1)}KB` : `${event.size}B`}
          </div>
          <div className="w-16 p-2 text-xs text-gray-500">{event.type}</div>
        </div>
      ))}
    </div>
  );
}

export function PerformanceComparison() {
  const [eventCount, setEventCount] = useState(100);
  const [useVirtualization, setUseVirtualization] = useState(true);
  
  const events = useMemo(() => generateSampleEvents(eventCount), [eventCount]);
  
  return (
    <div className="p-6 space-y-6">
      <div className="text-center">
        <h1 className="text-2xl font-bold mb-2">Network Event Table Performance Comparison</h1>
        <p className="text-gray-600">
          Compare traditional vs virtualized table performance with large datasets
        </p>
      </div>
      
      {/* Controls */}
      <div className="flex gap-4 items-center justify-center">
        <div>
          <label className="block text-sm font-medium mb-1">Event Count</label>
          <select 
            value={eventCount} 
            onChange={(e) => setEventCount(Number(e.target.value))}
            className="border rounded px-3 py-1"
          >
            <option value={100}>100 events</option>
            <option value={500}>500 events</option>
            <option value={1000}>1,000 events</option>
            <option value={5000}>5,000 events</option>
            <option value={10000}>10,000 events</option>
          </select>
        </div>
        
        <div>
          <label className="block text-sm font-medium mb-1">Implementation</label>
          <select 
            value={useVirtualization ? 'virtual' : 'traditional'} 
            onChange={(e) => setUseVirtualization(e.target.value === 'virtual')}
            className="border rounded px-3 py-1"
          >
            <option value="traditional">Traditional Table</option>
            <option value="virtual">Virtualized Table</option>
          </select>
        </div>
      </div>
      
      {/* Performance metrics */}
      <div className="bg-gray-50 p-4 rounded">
        <h3 className="font-medium mb-2">Performance Expectations</h3>
        <div className="grid grid-cols-2 gap-4 text-sm">
          <div>
            <strong>Traditional Table:</strong>
            <ul className="mt-1 space-y-1 text-gray-600">
              <li>• Renders all {eventCount} rows</li>
              <li>• Memory usage: ~{Math.round(eventCount * 0.5)}KB</li>
              <li>• Scroll performance: {eventCount > 1000 ? 'Poor' : 'Good'}</li>
            </ul>
          </div>
          <div>
            <strong>Virtualized Table:</strong>
            <ul className="mt-1 space-y-1 text-gray-600">
              <li>• Renders ~10-20 visible rows</li>
              <li>• Memory usage: ~5KB (constant)</li>
              <li>• Scroll performance: Excellent</li>
            </ul>
          </div>
        </div>
      </div>
      
      {/* Table */}
      <div>
        <h3 className="font-medium mb-2">
          {useVirtualization ? 'Virtualized' : 'Traditional'} Table 
          ({eventCount} events)
        </h3>
        
        {useVirtualization ? (
          <NetworkEventTable events={events} height={400} />
        ) : (
          <TraditionalTable events={events} />
        )}
      </div>
      
      {/* Recommendations */}
      <div className="bg-blue-50 p-4 rounded">
        <h3 className="font-medium text-blue-900 mb-2">Recommendations</h3>
        <ul className="space-y-1 text-blue-800 text-sm">
          <li>• Use virtualization for datasets larger than 100 rows</li>
          <li>• Always memoize row components and expensive computations</li>
          <li>• Consider react-window for simple lists, TanStack Virtual for complex needs</li>
          <li>• Test performance with your actual data patterns</li>
        </ul>
      </div>
    </div>
  );
}
