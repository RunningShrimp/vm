import React, { useState } from 'react';
import { Card, Button } from '../atoms';

interface ResourceMetric {
  timestamp: string;
  cpu: number;
  memory: number;
  disk: number;
  network: number;
}

interface PerformanceMetric {
  name: string;
  current: number;
  peak: number;
  average: number;
  unit: string;
}

export const PerformancePage: React.FC = () => {
  const [metrics, setMetrics] = useState<PerformanceMetric[]>([
    { name: 'CPU Usage', current: 45, peak: 92, average: 52, unit: '%' },
    { name: 'Memory Usage', current: 62, peak: 85, average: 68, unit: '%' },
    { name: 'Disk I/O', current: 120, peak: 450, average: 200, unit: 'MB/s' },
    { name: 'Network', current: 85, peak: 950, average: 320, unit: 'Mbps' },
    { name: 'Context Switches', current: 2500, peak: 5800, average: 3200, unit: '/sec' },
    { name: 'System Calls', current: 1200, peak: 3400, average: 1800, unit: '/sec' },
  ]);

  const [history, setHistory] = useState<ResourceMetric[]>([
    { timestamp: '10:00', cpu: 35, memory: 55, disk: 100, network: 65 },
    { timestamp: '10:05', cpu: 42, memory: 60, disk: 120, network: 85 },
    { timestamp: '10:10', cpu: 48, memory: 65, disk: 140, network: 95 },
    { timestamp: '10:15', cpu: 45, memory: 62, disk: 130, network: 88 },
    { timestamp: '10:20', cpu: 52, memory: 70, disk: 150, network: 110 },
    { timestamp: '10:25', cpu: 58, memory: 75, disk: 160, network: 120 },
    { timestamp: '10:30', cpu: 45, memory: 62, disk: 125, network: 85 },
  ]);

  const [selectedMetric, setSelectedMetric] = useState(0);
  const [timeRange, setTimeRange] = useState('1h');
  const [sortBy, setSortBy] = useState('current');

  const sortedMetrics = [...metrics].sort((a, b) => {
    if (sortBy === 'current') return b.current - a.current;
    if (sortBy === 'peak') return b.peak - a.peak;
    if (sortBy === 'average') return b.average - a.average;
    return 0;
  });

  const handleRefreshMetrics = () => {
    setMetrics(metrics.map(m => ({
      ...m,
      current: Math.max(0, m.current + (Math.random() - 0.5) * 20),
      peak: Math.max(m.peak, m.current),
      average: (m.average + m.current) / 2,
    })));
  };

  const handleExportData = () => {
    const data = history.map(h => 
      `${h.timestamp}, CPU: ${h.cpu}%, Memory: ${h.memory}%, Disk: ${h.disk}MB/s, Network: ${h.network}Mbps`
    ).join('\n');
    
    const element = document.createElement('a');
    element.setAttribute('href', 'data:text/plain;charset=utf-8,' + encodeURIComponent(data));
    element.setAttribute('download', `performance-metrics-${new Date().toISOString()}.txt`);
    element.style.display = 'none';
    document.body.appendChild(element);
    element.click();
    document.body.removeChild(element);
  };

  return (
    <div className="space-y-4 p-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-gray-800">Performance Analysis</h1>
          <p className="text-gray-500">System resource usage and performance metrics</p>
        </div>
        <div className="flex gap-2">
          <select
            value={timeRange}
            onChange={(e) => setTimeRange(e.target.value)}
            className="px-3 py-2 border rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
          >
            <option value="1h">1 Hour</option>
            <option value="6h">6 Hours</option>
            <option value="24h">24 Hours</option>
            <option value="7d">7 Days</option>
          </select>
          <Button
            onClick={handleRefreshMetrics}
            className="bg-blue-500 hover:bg-blue-600"
          >
            🔄 Refresh
          </Button>
          <Button
            onClick={handleExportData}
            className="bg-green-500 hover:bg-green-600"
          >
            📥 Export
          </Button>
        </div>
      </div>

      {/* Metrics Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {sortedMetrics.map((metric, idx) => {
          const percentage = Math.min(metric.current, 100);
          const getBarColor = () => {
            if (percentage >= 80) return 'bg-red-500';
            if (percentage >= 60) return 'bg-yellow-500';
            return 'bg-green-500';
          };

          return (
            <Card
              key={idx}
              onClick={() => setSelectedMetric(idx)}
              className={`p-4 cursor-pointer transition border-2 ${
                selectedMetric === idx
                  ? 'bg-blue-100 border-blue-500'
                  : 'bg-gray-50 hover:bg-gray-100 border-gray-200'
              }`}
            >
              <div className="flex items-center justify-between mb-2">
                <h3 className="font-semibold text-gray-800 text-sm">{metric.name}</h3>
                <span className="text-lg font-bold text-gray-800">
                  {metric.current.toFixed(1)}{metric.unit}
                </span>
              </div>

              <div className="mb-3">
                <div className="w-full bg-gray-200 rounded-full h-2">
                  <div
                    className={`h-2 rounded-full transition-all ${getBarColor()}`}
                    style={{ width: `${percentage}%` }}
                  />
                </div>
              </div>

              <div className="grid grid-cols-2 gap-2 text-xs">
                <div>
                  <p className="text-gray-500">Peak</p>
                  <p className="font-semibold text-gray-800">{metric.peak.toFixed(1)}{metric.unit}</p>
                </div>
                <div>
                  <p className="text-gray-500">Average</p>
                  <p className="font-semibold text-gray-800">{metric.average.toFixed(1)}{metric.unit}</p>
                </div>
              </div>
            </Card>
          );
        })}
      </div>

      {/* Detailed View */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        {/* Chart Placeholder */}
        <div className="lg:col-span-2">
          <Card className="p-4">
            <div className="flex items-center justify-between mb-4">
              <h2 className="font-semibold text-gray-800">
                {sortedMetrics[selectedMetric]?.name || 'Performance'} Trend
              </h2>
              <select
                value={sortBy}
                onChange={(e) => setSortBy(e.target.value)}
                className="px-3 py-1 border rounded text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
              >
                <option value="current">Sort by Current</option>
                <option value="peak">Sort by Peak</option>
                <option value="average">Sort by Average</option>
              </select>
            </div>

            <div className="bg-gray-100 rounded-lg p-8 text-center">
              <p className="text-gray-500 mb-2">Performance chart placeholder</p>
              <p className="text-xs text-gray-400">[Integration with Echarts coming soon]</p>
              <div className="mt-6 space-y-2">
                {history.map((h, i) => (
                  <div key={i} className="flex justify-between text-xs text-gray-600">
                    <span>{h.timestamp}</span>
                    <div className="flex gap-4">
                      <span className="text-blue-600">CPU: {h.cpu}%</span>
                      <span className="text-green-600">Mem: {h.memory}%</span>
                      <span className="text-purple-600">Disk: {h.disk}M</span>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </Card>
        </div>

        {/* Statistics */}
        <div className="space-y-4">
          {/* Summary */}
          <Card className="p-4">
            <h2 className="font-semibold text-gray-800 mb-3">Summary</h2>
            <div className="space-y-3 text-sm">
              <div className="flex justify-between pb-2 border-b">
                <span className="text-gray-600">Highest Load</span>
                <span className="font-semibold">{sortedMetrics[0]?.name}</span>
              </div>
              <div className="flex justify-between pb-2 border-b">
                <span className="text-gray-600">System Status</span>
                <span className="font-semibold text-green-600">✓ Normal</span>
              </div>
              <div className="flex justify-between pb-2 border-b">
                <span className="text-gray-600">Peak Time</span>
                <span className="font-semibold">10:25</span>
              </div>
              <div className="flex justify-between">
                <span className="text-gray-600">Avg Load</span>
                <span className="font-semibold">
                  {(metrics.reduce((sum, m) => sum + m.average, 0) / metrics.length).toFixed(1)}%
                </span>
              </div>
            </div>
          </Card>

          {/* Alerts */}
          <Card className="p-4">
            <h2 className="font-semibold text-gray-800 mb-3">Performance Alerts</h2>
            <div className="space-y-2 text-sm max-h-32 overflow-y-auto">
              <div className="p-2 bg-yellow-50 border-l-4 border-yellow-400 rounded">
                <p className="font-semibold text-yellow-800">⚠️ High CPU</p>
                <p className="text-yellow-700 text-xs">CPU usage 58% at 10:25</p>
              </div>
              <div className="p-2 bg-blue-50 border-l-4 border-blue-400 rounded">
                <p className="font-semibold text-blue-800">ℹ️ Info</p>
                <p className="text-blue-700 text-xs">Memory trend: +10% in 30 min</p>
              </div>
              <div className="p-2 bg-green-50 border-l-4 border-green-400 rounded">
                <p className="font-semibold text-green-800">✓ Normal</p>
                <p className="text-green-700 text-xs">Disk I/O within expected range</p>
              </div>
            </div>
          </Card>

          {/* Controls */}
          <Card className="p-4">
            <h2 className="font-semibold text-gray-800 mb-3">Controls</h2>
            <div className="space-y-2">
              <Button className="w-full bg-blue-500 hover:bg-blue-600 text-sm">
                📊 Detailed Report
              </Button>
              <Button className="w-full bg-purple-500 hover:bg-purple-600 text-sm">
                🎯 Set Alerts
              </Button>
              <Button className="w-full bg-gray-500 hover:bg-gray-600 text-sm">
                🔍 Analyze
              </Button>
            </div>
          </Card>
        </div>
      </div>
    </div>
  );
};
