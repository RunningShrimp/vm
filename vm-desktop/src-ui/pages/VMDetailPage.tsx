import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { Button, Badge, Tabs } from '../atoms';
import { CardLayout } from '../templates';
import { PerformanceDashboard } from '../components/PerformanceDashboard';
import { SnapshotManager } from '../components/SnapshotManager';

interface VM {
  id: string;
  name: string;
  state: 'Stopped' | 'Running' | 'Paused' | 'Suspended' | { Error: string };
  cpu_count: number;
  memory_mb: number;
  disk_gb: number;
  display_mode: 'GUI' | 'Terminal';
  os_type: 'Ubuntu' | 'Debian' | 'Windows' | 'CentOS' | 'Other';
}

interface VMDetailPageProps {
  vm: VM;
  loading?: boolean;
  onBack?: () => void;
  onStart?: (id: string) => void;
  onStop?: (id: string) => void;
  onPause?: (id: string) => void;
  onResume?: (id: string) => void;
}

export const VMDetailPage: React.FC<VMDetailPageProps> = ({
  vm,
  loading = false,
  onBack,
  onStart,
  onStop,
  onPause,
  onResume,
}) => {
  const [activeTab, setActiveTab] = useState<'overview' | 'performance' | 'snapshots' | 'console' | 'settings'>('overview');
  const [showCreateWizard, setShowCreateWizard] = useState(false);

  const handleStart = async () => {
    try {
      await invoke('start_vm', { id: vm.id });
      onStart?.(vm.id);
    } catch (error) {
      console.error('Failed to start VM:', error);
    }
  };

  const handleStop = async () => {
    try {
      await invoke('stop_vm', { id: vm.id });
      onStop?.(vm.id);
    } catch (error) {
      console.error('Failed to stop VM:', error);
    }
  };

  const handlePause = async () => {
    try {
      await invoke('pause_vm', { id: vm.id });
      onPause?.(vm.id);
    } catch (error) {
      console.error('Failed to pause VM:', error);
    }
  };

  const handleResume = async () => {
    try {
      await invoke('resume_vm', { id: vm.id });
      onResume?.(vm.id);
    } catch (error) {
      console.error('Failed to resume VM:', error);
    }
  };

  const getStatusColor = (state: string) => {
    switch (state) {
      case 'Running':
        return 'text-green-600 bg-green-100';
      case 'Paused':
        return 'text-yellow-600 bg-yellow-100';
      case 'Stopped':
        return 'text-gray-600 bg-gray-100';
      case 'Suspended':
        return 'text-orange-600 bg-orange-100';
      default:
        if (typeof state === 'object' && 'Error' in state) {
          return 'text-red-600 bg-red-100';
        }
        return 'text-gray-600 bg-gray-100';
    }
  };

  const getStatusText = (state: string) => {
    switch (state) {
      case 'Running':
        return '运行中';
      case 'Paused':
        return '已暂停';
      case 'Stopped':
        return '已停止';
      case 'Suspended':
        return '已挂起';
      default:
        if (typeof state === 'object' && 'Error' in state) {
          return `错误: ${(state as any).Error}`;
        }
        return '未知状态';
    }
  };

  const getOSIcon = (osType: string) => {
    switch (osType) {
      case 'Ubuntu':
        return '🟧';
      case 'Debian':
        return '🔷';
      case 'Windows':
        return '🪟';
      case 'CentOS':
        return '🔴';
      default:
        return '📦';
    }
  };

  const renderOverviewTab = () => (
    <div className="space-y-6">
      {/* VM Status Card */}
      <CardLayout title="虚拟机状态">
        <div className="flex items-center justify-between">
          <div className="flex items-center space-x-4">
            <span className="text-4xl">🖥️</span>
            <div>
              <h3 className="text-xl font-semibold text-gray-900">{vm.name}</h3>
              <p className="text-gray-600">ID: {vm.id}</p>
            </div>
          </div>
          <div className="text-right">
            <Badge className={getStatusColor(vm.state.toString())}>
              {getStatusText(vm.state.toString())}
            </Badge>
          </div>
        </div>
      </CardLayout>

      {/* VM Configuration */}
      <CardLayout title="配置信息">
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          <div className="space-y-4">
            <div>
              <h4 className="font-medium text-gray-900 mb-2">基本信息</h4>
              <dl className="space-y-1">
                <div className="flex justify-between">
                  <dt className="text-sm text-gray-500">操作系统:</dt>
                  <dd className="text-sm font-medium">
                    <span className="mr-2">{getOSIcon(vm.os_type)}</span>
                    {vm.os_type}
                  </dd>
                </div>
                <div className="flex justify-between">
                  <dt className="text-sm text-gray-500">显示模式:</dt>
                  <dd className="text-sm font-medium">{vm.display_mode}</dd>
                </div>
              </dl>
            </div>
          </div>

          <div className="space-y-4">
            <div>
              <h4 className="font-medium text-gray-900 mb-2">资源分配</h4>
              <dl className="space-y-1">
                <div className="flex justify-between">
                  <dt className="text-sm text-gray-500">CPU:</dt>
                  <dd className="text-sm font-medium">{vm.cpu_count} vCPUs</dd>
                </div>
                <div className="flex justify-between">
                  <dt className="text-sm text-gray-500">内存:</dt>
                  <dd className="text-sm font-medium">{vm.memory_mb} MB</dd>
                </div>
                <div className="flex justify-between">
                  <dt className="text-sm text-gray-500">磁盘:</dt>
                  <dd className="text-sm font-medium">{vm.disk_gb} GB</dd>
                </div>
              </dl>
            </div>
          </div>
        </div>
      </CardLayout>

      {/* Control Buttons */}
      <CardLayout title="控制操作">
        <div className="flex flex-wrap gap-3">
          {vm.state === 'Stopped' && (
            <Button variant="primary" onClick={handleStart}>
              ▶️ 启动
            </Button>
          )}
          
          {vm.state === 'Running' && (
            <>
              <Button variant="warning" onClick={handlePause}>
                ⏸️ 暂停
              </Button>
              <Button variant="danger" onClick={handleStop}>
                ⏹️ 停止
              </Button>
            </>
          )}
          
          {vm.state === 'Paused' && (
            <>
              <Button variant="success" onClick={handleResume}>
                ▶️ 恢复
              </Button>
              <Button variant="danger" onClick={handleStop}>
                ⏹️ 停止
              </Button>
            </>
          )}
        </div>
      </CardLayout>
    </div>
  );

  const renderPerformanceTab = () => (
    <div className="space-y-6">
      <PerformanceDashboard vmId={vm.id} vmName={vm.name} />
    </div>
  );

  const renderSnapshotsTab = () => (
    <div className="space-y-6">
      <SnapshotManager vmId={vm.id} vmName={vm.name} />
    </div>
  );

  const renderConsoleTab = () => (
    <div className="space-y-6">
      <CardLayout title="控制台">
        <div className="bg-black text-green-400 p-4 rounded-lg font-mono text-sm h-96 overflow-auto">
          <div>控制台功能将在未来版本中实现</div>
          <div>这将提供对VM串行端口的直接访问</div>
          <div>$ </div>
        </div>
      </CardLayout>
    </div>
  );

  const renderSettingsTab = () => (
    <div className="space-y-6">
      <CardLayout title="VM设置">
        <div className="space-y-4">
          <div>
            <h4 className="font-medium text-gray-900 mb-2">高级设置</h4>
            <p className="text-sm text-gray-600">
              VM高级设置功能将在未来版本中实现，包括：
            </p>
            <ul className="mt-2 ml-4 list-disc space-y-1 text-sm text-gray-600">
              <li>内核参数配置</li>
              <li>启动选项调整</li>
              <li>设备映射设置</li>
              <li>网络高级配置</li>
              <li>性能优化选项</li>
            </ul>
          </div>
        </div>
      </CardLayout>
    </div>
  );

  const tabs = [
    { id: 'overview', label: '概览', icon: '📊' },
    { id: 'performance', label: '性能监控', icon: '📈' },
    { id: 'snapshots', label: '快照管理', icon: '📸' },
    { id: 'console', label: '控制台', icon: '💻' },
    { id: 'settings', label: '设置', icon: '⚙️' },
  ];

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <button
          onClick={onBack}
          className="flex items-center text-gray-600 hover:text-gray-900"
        >
          ← 返回
        </button>
        <h2 className="text-2xl font-bold text-gray-900">虚拟机详情</h2>
        <div className="w-8"></div>
      </div>

      {/* VM Status Indicator */}
      <div className={`p-4 rounded-lg border ${
        vm.state === 'Running' ? 'bg-green-50 border-green-200' :
        vm.state === 'Paused' ? 'bg-yellow-50 border-yellow-200' :
        vm.state === 'Stopped' ? 'bg-gray-50 border-gray-200' :
        'bg-red-50 border-red-200'
      }`}>
        <div className="flex items-center">
          <div className={`w-3 h-3 rounded-full mr-3 ${
            vm.state === 'Running' ? 'bg-green-500' :
            vm.state === 'Paused' ? 'bg-yellow-500' :
            vm.state === 'Stopped' ? 'bg-gray-500' :
            'bg-red-500'
          }`}></div>
          <span className="font-medium">
            虚拟机当前状态: {getStatusText(vm.state.toString())}
          </span>
        </div>
      </div>

      {/* Tabs */}
      <Tabs
        tabs={tabs}
        activeTab={activeTab}
        onTabChange={setActiveTab}
      />

      {/* Tab Content */}
      <div className="mt-6">
        {activeTab === 'overview' && renderOverviewTab()}
        {activeTab === 'performance' && renderPerformanceTab()}
        {activeTab === 'snapshots' && renderSnapshotsTab()}
        {activeTab === 'console' && renderConsoleTab()}
        {activeTab === 'settings' && renderSettingsTab()}
      </div>
    </div>
  );
};