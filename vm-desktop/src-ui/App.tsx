import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { MainLayout, Sidebar, TopBar, SidebarItem } from './templates';
import { VMHomePage, VMDetailPage, SystemDashboard, SettingsPage } from './organisms';
import { VMListPage, MonitoringPage, AdvancedSettingsPage, CreateVMWizard, AlertsPage, TerminalPage, BackupPage, SnapshotPage, NetworkPage, TaskManagerPage, LogViewerPage, PerformancePage } from './pages';
import { PerformanceDashboard } from './components/PerformanceDashboard';
import { SnapshotManager } from './components/SnapshotManager';

interface VM {
  id: string;
  name: string;
  state: 'Stopped' | 'Running' | 'Paused' | 'Suspended' | { Error: string };
  cpu_count: number;
  memory_mb: number;
  display_mode: 'GUI' | 'Terminal';
  status?: string;
}

export default function App() {
  const [vms, setVms] = useState<VM[]>([]);
  const [selectedVmId, setSelectedVmId] = useState<string | null>(null);
  const [activePage, setActivePage] = useState('vms');
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    loadVms();
    const interval = setInterval(loadVms, 3000);
    return () => clearInterval(interval);
  }, []);

  const loadVms = async () => {
    try {
      setLoading(true);
      const vmList: VM[] = await invoke('list_vms');
      setVms(vmList.map(vm => ({
        ...vm,
        status: typeof vm.state === 'string' 
          ? vm.state.toLowerCase() 
          : vm.state === 'Running' ? 'running' : 'stopped'
      })));
    } catch (error) {
      console.error('Failed to load VMs:', error);
    } finally {
      setLoading(false);
    }
  };

  const handleCreateVM = () => {
    alert('Create VM feature coming soon');
  };

  const handleRefresh = () => {
    loadVms();
  };

  const currentVM = selectedVmId ? vms.find(vm => vm.id === selectedVmId) : null;

  const sidebarItems: SidebarItem[] = [
    { 
      id: 'vms', 
      label: '虚拟机', 
      icon: '🖥️',
      badge: vms.filter(vm => vm.status === 'running').length > 0 
        ? vms.filter(vm => vm.status === 'running').length.toString() 
        : undefined
    },
    { id: 'vm-list', label: '虚拟机列表', icon: '📋' },
    { id: 'monitoring', label: '监控告警', icon: '📊' },
    { id: 'performance', label: '性能分析', icon: '📈' },
    { id: 'dashboard', label: '仪表板', icon: '🎛️' },
    { id: 'alerts', label: '系统告警', icon: '🚨' },
    { id: 'tasks', label: '任务管理', icon: '✓' },
    { id: 'logs', label: '日志查看', icon: '📝' },
    { id: 'create-vm', label: '创建虚拟机', icon: '➕' },
    { id: 'backup', label: '备份恢复', icon: '💾' },
    { id: 'snapshots', label: '快照管理', icon: '📸' },
    { id: 'network', label: '网络配置', icon: '🌐' },
    { id: 'settings', label: '基础设置', icon: '⚙️' },
    { id: 'advanced', label: '高级设置', icon: '🔧' },
    { id: 'terminal', label: '终端', icon: '⌨️' },
  ];

  return (
    <MainLayout
      sidebar={
        <Sidebar
          items={sidebarItems}
          activeItem={activePage}
          onSelect={setActivePage}
          logo={
            <div className="flex items-center gap-2">
              <span className="text-2xl">🚀</span>
              <div>
                <div className="font-bold text-lg">VM Desktop</div>
                <div className="text-xs text-gray-400">v0.1.0</div>
              </div>
            </div>
          }
        />
      }
      header={
        <TopBar
          title={
            activePage === 'vms' ? '虚拟机' :
            activePage === 'vm-list' ? '虚拟机列表' :
            activePage === 'monitoring' ? '监控告警' :
            activePage === 'performance' ? '性能分析' :
            activePage === 'dashboard' ? '系统仪表板' :
            activePage === 'alerts' ? '系统告警' :
            activePage === 'tasks' ? '任务管理' :
            activePage === 'logs' ? '日志查看' :
            activePage === 'create-vm' ? '创建虚拟机' :
            activePage === 'backup' ? '备份恢复' :
            activePage === 'snapshots' ? '快照管理' :
            activePage === 'network' ? '网络配置' :
            activePage === 'settings' ? '基础设置' :
            activePage === 'advanced' ? '高级设置' :
            activePage === 'terminal' ? '终端' :
            'VM Desktop'
          }
          actions={
            (activePage === 'vms' || activePage === 'vm-list') && !selectedVmId && (
              <div className="flex gap-2">
                <button
                  onClick={handleRefresh}
                  className="px-4 py-2 rounded-lg bg-gray-100 hover:bg-gray-200 transition-colors text-sm"
                >
                  🔄 刷新
                </button>
                <button
                  onClick={handleCreateVM}
                  className="px-4 py-2 rounded-lg bg-blue-600 text-white hover:bg-blue-700 transition-colors text-sm font-medium"
                >
                  ➕ 新建虚拟机
                </button>
              </div>
            )
          }
        />
      }
      footer={
        <div className="flex justify-between items-center text-xs">
          <span>VM Desktop Tauri | Powered by React + Rust</span>
          <span>VMs: {vms.length} | Running: {vms.filter(vm => vm.status === 'running').length}</span>
        </div>
      }
    >
      {/* 虚拟机主页面 */}
      {activePage === 'vms' && !selectedVmId && (
        <VMHomePage
          vms={vms}
          loading={loading}
          onVMClick={setSelectedVmId}
          onCreateVM={handleCreateVM}
          onRefresh={handleRefresh}
        />
      )}

      {/* 虚拟机详情页面 */}
      {activePage === 'vms' && selectedVmId && currentVM && (
        <VMDetailPage
          vm={currentVM}
          loading={loading}
          onBack={() => setSelectedVmId(null)}
          onStart={() => console.log('Start VM')}
          onStop={() => console.log('Stop VM')}
        />
      )}

      {/* 虚拟机列表页面 */}
      {activePage === 'vm-list' && (
        <VMListPage
          vms={vms}
          loading={loading}
          onVMClick={setSelectedVmId}
          onCreateVM={handleCreateVM}
          onRefresh={handleRefresh}
        />
      )}

      {/* 监控告警页面 */}
      {activePage === 'monitoring' && (
        <MonitoringPage />
      )}

      {/* 仪表板页面 */}
      {activePage === 'dashboard' && (
        <SystemDashboard />
      )}

      {/* 系统告警页面 */}
      {activePage === 'alerts' && (
        <AlertsPage />
      )}

      {/* 创建虚拟机向导 */}
      {activePage === 'create-vm' && (
        <CreateVMWizard
          onComplete={() => {
            setActivePage('vms');
            loadVms();
          }}
          onCancel={() => setActivePage('vms')}
        />
      )}

      {/* 基础设置页面 */}
      {activePage === 'settings' && (
        <SettingsPage />
      )}

      {/* 高级设置页面 */}
      {activePage === 'advanced' && (
        <AdvancedSettingsPage />
      )}

      {/* 备份恢复页面 */}
      {activePage === 'backup' && (
        <BackupPage />
      )}

      {/* 快照管理页面 */}
      {activePage === 'snapshots' && (
        <SnapshotPage />
      )}

      {/* 网络配置页面 */}
      {activePage === 'network' && (
        <NetworkPage />
      )}

      {/* 任务管理页面 */}
      {activePage === 'tasks' && (
        <TaskManagerPage />
      )}

      {/* 日志查看页面 */}
      {activePage === 'logs' && (
        <LogViewerPage />
      )}

      {/* 性能分析页面 */}
      {activePage === 'performance' && (
        <PerformancePage />
      )}

      {/* 终端页面 */}
      {activePage === 'terminal' && (
        <TerminalPage />
      )}
    </MainLayout>
  );
}
