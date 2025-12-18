import React, { useState, useEffect } from 'react';
import { MetricsCard, ResourceBar } from '../molecules';
import { CardLayout, TabsLayout, GridLayout } from '../templates';

interface PerformanceMetrics {
  cpu_usage: number;
  memory_usage: number;
  disk_usage: number;
  network_in: number;
  network_out: number;
  disk_io_read: number;
  disk_io_write: number;
}

export const MonitoringPage: React.FC = () => {
  const [metrics, setMetrics] = useState<PerformanceMetrics>({
    cpu_usage: 45,
    memory_usage: 2048,
    disk_usage: 250,
    network_in: 125,
    network_out: 87,
    disk_io_read: 156,
    disk_io_write: 89,
  });

  const [timeRange, setTimeRange] = useState('1h');
  const [autoRefresh, setAutoRefresh] = useState(true);

  useEffect(() => {
    if (!autoRefresh) return;

    const interval = setInterval(() => {
      setMetrics(prev => ({
        cpu_usage: Math.max(10, Math.min(100, prev.cpu_usage + (Math.random() - 0.5) * 10)),
        memory_usage: Math.max(1024, Math.min(8192, prev.memory_usage + (Math.random() - 0.5) * 256)),
        disk_usage: prev.disk_usage,
        network_in: Math.max(0, prev.network_in + (Math.random() - 0.5) * 50),
        network_out: Math.max(0, prev.network_out + (Math.random() - 0.5) * 50),
        disk_io_read: Math.max(0, prev.disk_io_read + (Math.random() - 0.5) * 100),
        disk_io_write: Math.max(0, prev.disk_io_write + (Math.random() - 0.5) * 100),
      }));
    }, 2000);

    return () => clearInterval(interval);
  }, [autoRefresh]);

  const tabs = [
    { label: '总体概览', value: 'overview' },
    { label: 'CPU 性能', value: 'cpu' },
    { label: '内存性能', value: 'memory' },
    { label: '网络监控', value: 'network' },
    { label: '磁盘 I/O', value: 'disk' },
  ];

  const [activeTab, setActiveTab] = useState('overview');

  return (
    <div className="space-y-6">
      {/* 控制栏 */}
      <div className="flex justify-between items-center bg-white p-4 rounded-lg border border-gray-200">
        <div className="flex gap-2">
          <button
            onClick={() => setTimeRange('1h')}
            className={`px-4 py-2 rounded text-sm font-medium transition-colors ${
              timeRange === '1h'
                ? 'bg-blue-600 text-white'
                : 'bg-gray-100 text-gray-900 hover:bg-gray-200'
            }`}
          >
            1 小时
          </button>
          <button
            onClick={() => setTimeRange('24h')}
            className={`px-4 py-2 rounded text-sm font-medium transition-colors ${
              timeRange === '24h'
                ? 'bg-blue-600 text-white'
                : 'bg-gray-100 text-gray-900 hover:bg-gray-200'
            }`}
          >
            24 小时
          </button>
          <button
            onClick={() => setTimeRange('7d')}
            className={`px-4 py-2 rounded text-sm font-medium transition-colors ${
              timeRange === '7d'
                ? 'bg-blue-600 text-white'
                : 'bg-gray-100 text-gray-900 hover:bg-gray-200'
            }`}
          >
            7 天
          </button>
        </div>
        <label className="flex items-center gap-2 cursor-pointer">
          <input
            type="checkbox"
            checked={autoRefresh}
            onChange={e => setAutoRefresh(e.target.checked)}
            className="w-4 h-4"
          />
          <span className="text-sm text-gray-700">自动刷新</span>
        </label>
      </div>

      {/* 标签页 */}
      <TabsLayout
        tabs={[
          {
            label: '总体概览',
            value: 'overview',
            icon: '📊',
            content: (
              <div className="space-y-6">
                {/* 关键指标 */}
                <GridLayout columns={4} gap="md">
                  <MetricsCard
                    title="CPU 使用率"
                    value={Math.round(metrics.cpu_usage)}
                    unit="%"
                    icon="⚙️"
                    trend={Math.random() > 0.5 ? 'up' : 'down'}
                  />
                  <MetricsCard
                    title="内存使用"
                    value={Math.round(metrics.memory_usage / 1024)}
                    unit="GB"
                    icon="💾"
                    trend="stable"
                  />
                  <MetricsCard
                    title="磁盘使用"
                    value={Math.round((metrics.disk_usage / 500) * 100)}
                    unit="%"
                    icon="💿"
                  />
                  <MetricsCard
                    title="网络(进)"
                    value={Math.round(metrics.network_in)}
                    unit="Mbps"
                    icon="🌐"
                  />
                </GridLayout>

                {/* 资源条 */}
                <CardLayout title="资源使用详情">
                  <div className="space-y-4">
                    <ResourceBar
                      label="CPU"
                      used={metrics.cpu_usage}
                      total={100}
                      unit="%"
                    />
                    <ResourceBar
                      label="内存"
                      used={metrics.memory_usage}
                      total={8192}
                      unit="MB"
                    />
                    <ResourceBar
                      label="磁盘"
                      used={metrics.disk_usage}
                      total={500}
                      unit="GB"
                    />
                  </div>
                </CardLayout>

                {/* 性能趋势 */}
                <div className="grid grid-cols-2 gap-6">
                  <CardLayout title="CPU 趋势">
                    <div className="h-40 bg-gray-50 rounded flex items-center justify-center text-gray-400">
                      [CPU 性能趋势图表]
                    </div>
                  </CardLayout>
                  <CardLayout title="内存趋势">
                    <div className="h-40 bg-gray-50 rounded flex items-center justify-center text-gray-400">
                      [内存性能趋势图表]
                    </div>
                  </CardLayout>
                </div>
              </div>
            ),
          },
          {
            label: 'CPU 性能',
            value: 'cpu',
            icon: '⚙️',
            content: (
              <div className="space-y-6">
                <CardLayout title="CPU 使用率">
                  <div className="space-y-4">
                    <ResourceBar
                      label="总体 CPU"
                      used={metrics.cpu_usage}
                      total={100}
                      unit="%"
                    />
                    <div className="grid grid-cols-4 gap-2">
                      {[1, 2, 3, 4].map(i => (
                        <div key={i} className="bg-gray-50 p-3 rounded">
                          <div className="text-sm font-medium text-gray-600">核心 {i}</div>
                          <div className="text-lg font-bold text-blue-600">
                            {Math.round(metrics.cpu_usage + Math.random() * 10)}%
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                </CardLayout>

                <CardLayout title="CPU 详细信息">
                  <div className="space-y-2 text-sm">
                    <div className="flex justify-between">
                      <span className="text-gray-600">核心数</span>
                      <span className="font-medium">8</span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-gray-600">频率</span>
                      <span className="font-medium">2.4 GHz</span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-gray-600">缓存</span>
                      <span className="font-medium">16 MB</span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-gray-600">温度</span>
                      <span className="font-medium">45°C</span>
                    </div>
                  </div>
                </CardLayout>
              </div>
            ),
          },
          {
            label: '内存性能',
            value: 'memory',
            icon: '💾',
            content: (
              <div className="space-y-6">
                <CardLayout title="内存使用">
                  <div className="space-y-4">
                    <ResourceBar
                      label="已用"
                      used={metrics.memory_usage}
                      total={8192}
                      unit="MB"
                    />
                    <div className="grid grid-cols-2 gap-4">
                      <div className="bg-blue-50 p-4 rounded">
                        <div className="text-sm text-gray-600">已分配</div>
                        <div className="text-2xl font-bold text-blue-600">
                          {Math.round(metrics.memory_usage / 1024)}/{8} GB
                        </div>
                      </div>
                      <div className="bg-green-50 p-4 rounded">
                        <div className="text-sm text-gray-600">可用</div>
                        <div className="text-2xl font-bold text-green-600">
                          {Math.round((8192 - metrics.memory_usage) / 1024)} GB
                        </div>
                      </div>
                    </div>
                  </div>
                </CardLayout>

                <CardLayout title="内存分布">
                  <div className="space-y-2 text-sm">
                    <div className="flex justify-between">
                      <span className="text-gray-600">应用</span>
                      <span className="font-medium">3.2 GB</span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-gray-600">缓存</span>
                      <span className="font-medium">1.5 GB</span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-gray-600">缓冲</span>
                      <span className="font-medium">0.8 GB</span>
                    </div>
                  </div>
                </CardLayout>
              </div>
            ),
          },
          {
            label: '网络监控',
            value: 'network',
            icon: '🌐',
            content: (
              <div className="space-y-6">
                <GridLayout columns={2} gap="lg">
                  <CardLayout title="网络进速">
                    <div className="text-4xl font-bold text-blue-600 mb-2">
                      {Math.round(metrics.network_in)} Mbps
                    </div>
                    <div className="text-sm text-gray-600">下载速度</div>
                  </CardLayout>
                  <CardLayout title="网络出速">
                    <div className="text-4xl font-bold text-green-600 mb-2">
                      {Math.round(metrics.network_out)} Mbps
                    </div>
                    <div className="text-sm text-gray-600">上传速度</div>
                  </CardLayout>
                </GridLayout>

                <CardLayout title="网络接口">
                  <div className="space-y-3">
                    {['eth0', 'eth1', 'virt-net0'].map(intf => (
                      <div key={intf} className="border border-gray-200 p-3 rounded">
                        <div className="font-medium text-gray-900 mb-2">{intf}</div>
                        <div className="space-y-2 text-sm">
                          <div className="flex justify-between">
                            <span className="text-gray-600">接收</span>
                            <span className="font-medium">{Math.round(Math.random() * 1000)} MB</span>
                          </div>
                          <div className="flex justify-between">
                            <span className="text-gray-600">发送</span>
                            <span className="font-medium">{Math.round(Math.random() * 800)} MB</span>
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                </CardLayout>
              </div>
            ),
          },
          {
            label: '磁盘 I/O',
            value: 'disk',
            icon: '💿',
            content: (
              <div className="space-y-6">
                <GridLayout columns={2} gap="lg">
                  <CardLayout title="读速度">
                    <div className="text-4xl font-bold text-blue-600 mb-2">
                      {Math.round(metrics.disk_io_read)} MB/s
                    </div>
                    <div className="text-sm text-gray-600">磁盘读取</div>
                  </CardLayout>
                  <CardLayout title="写速度">
                    <div className="text-4xl font-bold text-orange-600 mb-2">
                      {Math.round(metrics.disk_io_write)} MB/s
                    </div>
                    <div className="text-sm text-gray-600">磁盘写入</div>
                  </CardLayout>
                </GridLayout>

                <CardLayout title="磁盘使用">
                  <div className="space-y-4">
                    <div>
                      <div className="flex justify-between mb-2">
                        <span className="text-sm font-medium">/dev/sda</span>
                        <span className="text-sm text-gray-600">250 GB / 500 GB</span>
                      </div>
                      <div className="w-full bg-gray-200 rounded-full h-2">
                        <div
                          className="bg-blue-600 h-2 rounded-full"
                          style={{ width: '50%' }}
                        ></div>
                      </div>
                    </div>
                    <div>
                      <div className="flex justify-between mb-2">
                        <span className="text-sm font-medium">/dev/sdb</span>
                        <span className="text-sm text-gray-600">180 GB / 200 GB</span>
                      </div>
                      <div className="w-full bg-gray-200 rounded-full h-2">
                        <div
                          className="bg-orange-600 h-2 rounded-full"
                          style={{ width: '90%' }}
                        ></div>
                      </div>
                    </div>
                  </div>
                </CardLayout>
              </div>
            ),
          },
        ]}
        activeTab={activeTab}
        onChange={setActiveTab}
      />
    </div>
  );
};
