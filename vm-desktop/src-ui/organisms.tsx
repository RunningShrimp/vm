/**
 * 生物体组件库 - 完整页面模块
 * 模仿 VMware Workstation 的核心页面
 */

import React from 'react';
import { VMCard, MetricsCard, ResourceBar, VMToolbar, EmptyState, LoadingPlaceholder, Modal, ConfirmDialog } from './molecules';
import { Button, Tabs } from './atoms';

/** 虚拟机主页面 */
export interface VMHomePageProps {
  vms: any[];
  loading?: boolean;
  onVMClick?: (id: string) => void;
  onCreateVM?: () => void;
  onRefresh?: () => void;
}

export const VMHomePage: React.FC<VMHomePageProps> = ({
  vms,
  loading = false,
  onVMClick,
  onCreateVM,
  onRefresh,
}) => {
  const runningCount = vms.filter((vm) => vm.status === 'running').length;
  const stoppedCount = vms.filter((vm) => vm.status === 'stopped').length;

  return (
    <div className="space-y-6">
      {/* 统计卡片 */}
      <div className="grid grid-cols-4 gap-4">
        <MetricsCard title="总虚拟机数" value={vms.length} icon="🖥️" />
        <MetricsCard title="运行中" value={runningCount} icon="▶️" trend="stable" />
        <MetricsCard title="已停止" value={stoppedCount} icon="⏹️" />
        <MetricsCard title="CPU 使用率" value="45" unit="%" icon="⚙️" trend="down" />
      </div>

      {/* 工具栏 */}
      <VMToolbar onNew={onCreateVM} onRefresh={onRefresh} selectedCount={0} />

      {/* VM 列表 */}
      {loading ? (
        <LoadingPlaceholder message="加载虚拟机列表..." />
      ) : vms.length === 0 ? (
        <EmptyState
          icon="📭"
          title="还没有虚拟机"
          description="创建您的第一个虚拟机以开始使用"
          action={<Button variant="primary" onClick={onCreateVM}>创建虚拟机</Button>}
        />
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {vms.map((vm) => (
            <VMCard
              key={vm.id}
              {...vm}
              onClick={() => onVMClick?.(vm.id)}
            />
          ))}
        </div>
      )}
    </div>
  );
};

/** 虚拟机详情页面 */
export interface VMDetailPageProps {
  vm: any;
  loading?: boolean;
  metrics?: any;
  onBack?: () => void;
  onStart?: () => void;
  onStop?: () => void;
}

export const VMDetailPage: React.FC<VMDetailPageProps> = ({
  vm,
  loading = false,
  metrics = {},
  onBack,
  onStart,
  onStop,
}) => {
  const [activeTab, setActiveTab] = React.useState('overview');

  const tabs = [
    { label: '概览', value: 'overview' },
    { label: '性能', value: 'performance' },
    { label: '配置', value: 'settings' },
    { label: '日志', value: 'logs' },
  ];

  return (
    <div className="space-y-6">
      {/* 返回按钮和标题 */}
      <div className="flex items-center gap-4">
        <Button variant="ghost" onClick={onBack}>
          ← 返回
        </Button>
        <div className="flex-1">
          <h1 className="text-3xl font-bold text-gray-900">{vm.name}</h1>
          <p className="text-gray-600">ID: {vm.id}</p>
        </div>
        <div className="flex gap-2">
          {vm.status === 'stopped' && (
            <Button variant="success" onClick={onStart}>启动</Button>
          )}
          {vm.status === 'running' && (
            <Button variant="danger" onClick={onStop}>停止</Button>
          )}
        </div>
      </div>

      {/* 标签页 */}
      <Tabs tabs={tabs} activeTab={activeTab} onChange={setActiveTab} />

      {/* 概览标签页 */}
      {activeTab === 'overview' && (
        <div className="grid grid-cols-2 gap-6">
          <div className="space-y-4">
            <h3 className="text-lg font-semibold">虚拟机信息</h3>
            <div className="space-y-2 text-sm">
              <div className="flex justify-between">
                <span className="text-gray-600">CPU 核心</span>
                <span className="font-medium">{vm.cpu_count}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-gray-600">内存</span>
                <span className="font-medium">{vm.memory_mb} MB</span>
              </div>
              <div className="flex justify-between">
                <span className="text-gray-600">显示模式</span>
                <span className="font-medium">{vm.display_mode}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-gray-600">状态</span>
                <span className="font-medium">{vm.state}</span>
              </div>
            </div>
          </div>

          <div className="space-y-4">
            <h3 className="text-lg font-semibold">快速统计</h3>
            <div className="grid grid-cols-2 gap-2">
              <MetricsCard title="CPU" value="45" unit="%" icon="⚙️" />
              <MetricsCard title="内存" value="2.1" unit="GB" icon="💾" />
              <MetricsCard title="磁盘" value="15.3" unit="GB" icon="💿" />
              <MetricsCard title="网络" value="2.5" unit="Mbps" icon="🌐" />
            </div>
          </div>
        </div>
      )}

      {/* 性能标签页 */}
      {activeTab === 'performance' && (
        <div className="space-y-6">
          <h3 className="text-lg font-semibold">资源使用情况</h3>
          <ResourceBar label="CPU 使用率" used={45} total={100} unit="%" />
          <ResourceBar label="内存使用" used={2048} total={4096} unit="MB" />
          <ResourceBar label="磁盘使用" used={50} total={100} unit="GB" />
        </div>
      )}

      {/* 配置标签页 */}
      {activeTab === 'settings' && (
        <div className="space-y-4">
          <h3 className="text-lg font-semibold">虚拟机配置</h3>
          <div className="bg-gray-50 p-4 rounded-lg space-y-3">
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">CPU 核心数</label>
              <input type="number" defaultValue={vm.cpu_count} className="w-full px-3 py-2 border border-gray-300 rounded-md" />
            </div>
            <div>
              <label className="block text-sm font-medium text-gray-700 mb-1">内存 (MB)</label>
              <input type="number" defaultValue={vm.memory_mb} className="w-full px-3 py-2 border border-gray-300 rounded-md" />
            </div>
            <div className="flex gap-2 pt-4">
              <Button variant="primary">保存更改</Button>
              <Button variant="outline">取消</Button>
            </div>
          </div>
        </div>
      )}

      {/* 日志标签页 */}
      {activeTab === 'logs' && (
        <div className="space-y-4">
          <h3 className="text-lg font-semibold">虚拟机日志</h3>
          <div className="bg-gray-900 text-green-400 p-4 rounded-lg font-mono text-sm max-h-96 overflow-y-auto">
            <div>[2025-12-11 10:30:45] 虚拟机启动</div>
            <div>[2025-12-11 10:30:50] 加载 BIOS</div>
            <div>[2025-12-11 10:31:00] 初始化系统</div>
            <div>[2025-12-11 10:31:15] 启动完成</div>
            <div className="text-gray-400">[等待更多日志...]</div>
          </div>
        </div>
      )}
    </div>
  );
};

/** 系统仪表板 */
export const SystemDashboard: React.FC = () => {
  const [activeTab, setActiveTab] = React.useState('overview');

  const tabs = [
    { label: '概览', value: 'overview' },
    { label: '性能', value: 'performance' },
    { label: '网络', value: 'network' },
    { label: '存储', value: 'storage' },
  ];

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold text-gray-900">系统仪表板</h1>
        <p className="text-gray-600">实时监控您的虚拟化环境</p>
      </div>

      {/* 统计卡片 */}
      <div className="grid grid-cols-4 gap-4">
        <MetricsCard title="总 CPU" value="8" icon="⚙️" trend="stable" />
        <MetricsCard title="总内存" value="32" unit="GB" icon="💾" trend="up" />
        <MetricsCard title="已分配" value="24" unit="GB" icon="📊" />
        <MetricsCard title="运行时间" value="45" unit="天" icon="⏱️" trend="stable" />
      </div>

      {/* 标签页 */}
      <Tabs tabs={tabs} activeTab={activeTab} onChange={setActiveTab} />

      {/* 概览 */}
      {activeTab === 'overview' && (
        <div className="grid grid-cols-2 gap-6">
          <div>
            <h3 className="text-lg font-semibold mb-4">物理资源</h3>
            <div className="space-y-4">
              <ResourceBar label="CPU 总体使用" used={6} total={8} unit="核心" />
              <ResourceBar label="内存总体使用" used={24} total={32} unit="GB" />
              <ResourceBar label="磁盘总体使用" used={450} total={500} unit="GB" />
            </div>
          </div>
          <div>
            <h3 className="text-lg font-semibold mb-4">虚拟机状态</h3>
            <div className="space-y-2 text-sm">
              <div className="flex justify-between bg-gray-50 p-3 rounded">
                <span>运行中</span>
                <span className="font-bold text-green-600">5 台</span>
              </div>
              <div className="flex justify-between bg-gray-50 p-3 rounded">
                <span>已停止</span>
                <span className="font-bold text-gray-600">3 台</span>
              </div>
              <div className="flex justify-between bg-gray-50 p-3 rounded">
                <span>暂停中</span>
                <span className="font-bold text-yellow-600">1 台</span>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* 性能 */}
      {activeTab === 'performance' && (
        <div className="space-y-6">
          <div className="bg-gray-50 p-6 rounded-lg">
            <h3 className="text-lg font-semibold mb-4">CPU 性能趋势</h3>
            <div className="h-40 bg-white rounded border border-gray-200 flex items-center justify-center text-gray-400">
              [性能图表区域]
            </div>
          </div>
          <div className="bg-gray-50 p-6 rounded-lg">
            <h3 className="text-lg font-semibold mb-4">内存性能趋势</h3>
            <div className="h-40 bg-white rounded border border-gray-200 flex items-center justify-center text-gray-400">
              [性能图表区域]
            </div>
          </div>
        </div>
      )}

      {/* 网络 */}
      {activeTab === 'network' && (
        <div className="space-y-4">
          <h3 className="text-lg font-semibold">网络接口</h3>
          <div className="space-y-2">
            {['eth0', 'eth1', 'virt-net0'].map((intf) => (
              <div key={intf} className="bg-gray-50 p-4 rounded-lg">
                <div className="flex justify-between mb-2">
                  <span className="font-medium">{intf}</span>
                  <span className="text-sm text-gray-600">Active</span>
                </div>
                <ResourceBar label="下载速度" used={25} total={100} unit="Mbps" />
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 存储 */}
      {activeTab === 'storage' && (
        <div className="space-y-4">
          <h3 className="text-lg font-semibold">存储设备</h3>
          <div className="space-y-2">
            {[
              { name: '/dev/sda', used: 350, total: 500 },
              { name: '/dev/sdb', used: 100, total: 200 },
            ].map((disk) => (
              <div key={disk.name} className="bg-gray-50 p-4 rounded-lg">
                <div className="font-medium mb-2">{disk.name}</div>
                <ResourceBar label="使用空间" used={disk.used} total={disk.total} unit="GB" />
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};

/** 设置页面 */
export const SettingsPage: React.FC = () => {
  const [activeTab, setActiveTab] = React.useState('general');

  const tabs = [
    { label: '常规', value: 'general' },
    { label: '显示', value: 'display' },
    { label: '热键', value: 'hotkeys' },
    { label: '关于', value: 'about' },
  ];

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold text-gray-900">设置</h1>
      </div>

      <Tabs tabs={tabs} activeTab={activeTab} onChange={setActiveTab} />

      {/* 常规设置 */}
      {activeTab === 'general' && (
        <div className="space-y-4 max-w-2xl">
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">默认虚拟机路径</label>
            <input type="text" defaultValue="/home/user/vms" className="w-full px-3 py-2 border border-gray-300 rounded-md" />
          </div>
          <div>
            <label className="flex items-center gap-2 text-sm font-medium text-gray-700">
              <input type="checkbox" defaultChecked />
              启动时自动运行最后使用的虚拟机
            </label>
          </div>
          <div className="flex gap-2 pt-4">
            <Button variant="primary">保存</Button>
            <Button variant="outline">恢复默认</Button>
          </div>
        </div>
      )}

      {/* 显示设置 */}
      {activeTab === 'display' && (
        <div className="space-y-4 max-w-2xl">
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">主题</label>
            <select className="w-full px-3 py-2 border border-gray-300 rounded-md">
              <option>浅色</option>
              <option>深色</option>
              <option>自动</option>
            </select>
          </div>
          <div className="flex gap-2 pt-4">
            <Button variant="primary">应用</Button>
          </div>
        </div>
      )}

      {/* 热键设置 */}
      {activeTab === 'hotkeys' && (
        <div className="space-y-4 max-w-2xl">
          <div className="space-y-2 text-sm">
            <div className="flex justify-between bg-gray-50 p-3 rounded">
              <span>启动/停止</span>
              <code className="text-xs">Ctrl+Enter</code>
            </div>
            <div className="flex justify-between bg-gray-50 p-3 rounded">
              <span>暂停/恢复</span>
              <code className="text-xs">Ctrl+P</code>
            </div>
            <div className="flex justify-between bg-gray-50 p-3 rounded">
              <span>全屏</span>
              <code className="text-xs">F11</code>
            </div>
          </div>
        </div>
      )}

      {/* 关于 */}
      {activeTab === 'about' && (
        <div className="space-y-4 max-w-2xl">
          <div className="bg-gray-50 p-6 rounded-lg text-center">
            <h3 className="text-2xl font-bold mb-2">VM Desktop</h3>
            <p className="text-gray-600 mb-4">Tauri 跨平台虚拟机管理器</p>
            <p className="text-sm text-gray-500 space-y-1">
              <div>版本: 0.1.0</div>
              <div>Tauri 2.0 | React 18 | Rust</div>
              <div className="mt-4">© 2025 VM 开发团队</div>
            </p>
          </div>
        </div>
      )}
    </div>
  );
};
