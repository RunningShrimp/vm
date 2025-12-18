import React, { useState } from 'react';
import { Button, Input, Badge } from '../atoms';
import { CardLayout, FormLayout } from '../templates';

export const AdvancedSettingsPage: React.FC = () => {
  const [activeSection, setActiveSection] = useState('general');
  const [settings, setSettings] = useState({
    vmPath: '/home/user/vms',
    autoStart: false,
    autoSnapshot: true,
    snapshotInterval: 30,
    enableGPU: true,
    maxCPUOvercommit: 200,
    maxMemoryOvercommit: 150,
    enableClusterMode: false,
    enableAutoBackup: true,
    backupInterval: 24,
    backupPath: '/home/user/backups',
    networkBridge: 'br0',
    enableNetworkOptimization: true,
    storagePool: 'default',
    enableCompressionBackup: true,
    logLevel: 'info',
    enableRemoteManagement: false,
    remotePort: 8080,
  });

  const sections = [
    { id: 'general', label: '常规设置', icon: '⚙️' },
    { id: 'performance', label: '性能优化', icon: '⚡' },
    { id: 'backup', label: '备份还原', icon: '💾' },
    { id: 'network', label: '网络配置', icon: '🌐' },
    { id: 'advanced', label: '高级选项', icon: '🔧' },
  ];

  const handleSettingChange = (key: string, value: any) => {
    setSettings(prev => ({
      ...prev,
      [key]: value,
    }));
  };

  const handleSave = () => {
    console.log('Saving settings:', settings);
    alert('设置已保存');
  };

  const handleReset = () => {
    if (confirm('确认恢复默认设置？')) {
      setSettings({
        vmPath: '/home/user/vms',
        autoStart: false,
        autoSnapshot: true,
        snapshotInterval: 30,
        enableGPU: true,
        maxCPUOvercommit: 200,
        maxMemoryOvercommit: 150,
        enableClusterMode: false,
        enableAutoBackup: true,
        backupInterval: 24,
        backupPath: '/home/user/backups',
        networkBridge: 'br0',
        enableNetworkOptimization: true,
        storagePool: 'default',
        enableCompressionBackup: true,
        logLevel: 'info',
        enableRemoteManagement: false,
        remotePort: 8080,
      });
    }
  };

  const renderContent = () => {
    switch (activeSection) {
      case 'general':
        return (
          <div className="space-y-6">
            <CardLayout title="基本设置">
              <div className="space-y-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">
                    虚拟机存储路径
                  </label>
                  <Input
                    value={settings.vmPath}
                    onChange={e => handleSettingChange('vmPath', e.target.value)}
                    placeholder="/home/user/vms"
                  />
                </div>
                <label className="flex items-center gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.autoStart}
                    onChange={e => handleSettingChange('autoStart', e.target.checked)}
                    className="w-4 h-4"
                  />
                  <span className="text-sm font-medium text-gray-700">
                    启动时自动运行上次使用的虚拟机
                  </span>
                </label>
                <label className="flex items-center gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.autoSnapshot}
                    onChange={e => handleSettingChange('autoSnapshot', e.target.checked)}
                    className="w-4 h-4"
                  />
                  <span className="text-sm font-medium text-gray-700">
                    启用自动快照
                  </span>
                </label>
                {settings.autoSnapshot && (
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">
                      快照间隔 (分钟)
                    </label>
                    <input
                      type="number"
                      value={settings.snapshotInterval}
                      onChange={e => handleSettingChange('snapshotInterval', parseInt(e.target.value))}
                      className="w-full px-3 py-2 border border-gray-300 rounded-md"
                    />
                  </div>
                )}
              </div>
            </CardLayout>
          </div>
        );

      case 'performance':
        return (
          <div className="space-y-6">
            <CardLayout title="硬件配置">
              <div className="space-y-4">
                <label className="flex items-center gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.enableGPU}
                    onChange={e => handleSettingChange('enableGPU', e.target.checked)}
                    className="w-4 h-4"
                  />
                  <span className="text-sm font-medium text-gray-700">
                    启用 GPU 加速
                  </span>
                  <Badge variant="success">可用</Badge>
                </label>
              </div>
            </CardLayout>

            <CardLayout title="资源超配策略">
              <div className="space-y-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">
                    CPU 超配比例: {settings.maxCPUOvercommit}%
                  </label>
                  <input
                    type="range"
                    min="100"
                    max="400"
                    step="10"
                    value={settings.maxCPUOvercommit}
                    onChange={e => handleSettingChange('maxCPUOvercommit', parseInt(e.target.value))}
                    className="w-full"
                  />
                  <div className="text-xs text-gray-600 mt-1">
                    允许虚拟 CPU 总数超过物理 CPU 总数的比例
                  </div>
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">
                    内存超配比例: {settings.maxMemoryOvercommit}%
                  </label>
                  <input
                    type="range"
                    min="100"
                    max="300"
                    step="10"
                    value={settings.maxMemoryOvercommit}
                    onChange={e => handleSettingChange('maxMemoryOvercommit', parseInt(e.target.value))}
                    className="w-full"
                  />
                  <div className="text-xs text-gray-600 mt-1">
                    允许虚拟内存总数超过物理内存总数的比例
                  </div>
                </div>
              </div>
            </CardLayout>

            <CardLayout title="集群模式">
              <div className="space-y-4">
                <label className="flex items-center gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.enableClusterMode}
                    onChange={e => handleSettingChange('enableClusterMode', e.target.checked)}
                    className="w-4 h-4"
                  />
                  <span className="text-sm font-medium text-gray-700">
                    启用集群模式
                  </span>
                  <Badge variant="warning">实验性</Badge>
                </label>
              </div>
            </CardLayout>
          </div>
        );

      case 'backup':
        return (
          <div className="space-y-6">
            <CardLayout title="自动备份">
              <div className="space-y-4">
                <label className="flex items-center gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.enableAutoBackup}
                    onChange={e => handleSettingChange('enableAutoBackup', e.target.checked)}
                    className="w-4 h-4"
                  />
                  <span className="text-sm font-medium text-gray-700">
                    启用自动备份
                  </span>
                </label>
                {settings.enableAutoBackup && (
                  <>
                    <div>
                      <label className="block text-sm font-medium text-gray-700 mb-1">
                        备份间隔 (小时)
                      </label>
                      <input
                        type="number"
                        value={settings.backupInterval}
                        onChange={e => handleSettingChange('backupInterval', parseInt(e.target.value))}
                        className="w-full px-3 py-2 border border-gray-300 rounded-md"
                      />
                    </div>
                    <div>
                      <label className="block text-sm font-medium text-gray-700 mb-1">
                        备份路径
                      </label>
                      <Input
                        value={settings.backupPath}
                        onChange={e => handleSettingChange('backupPath', e.target.value)}
                        placeholder="/home/user/backups"
                      />
                    </div>
                  </>
                )}
              </div>
            </CardLayout>

            <CardLayout title="备份选项">
              <div className="space-y-4">
                <label className="flex items-center gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.enableCompressionBackup}
                    onChange={e => handleSettingChange('enableCompressionBackup', e.target.checked)}
                    className="w-4 h-4"
                  />
                  <span className="text-sm font-medium text-gray-700">
                    启用备份压缩
                  </span>
                </label>
              </div>
            </CardLayout>

            <CardLayout title="快速操作">
              <div className="space-y-2">
                <Button variant="outline" className="w-full">
                  📤 立即备份
                </Button>
                <Button variant="outline" className="w-full">
                  📥 还原备份
                </Button>
              </div>
            </CardLayout>
          </div>
        );

      case 'network':
        return (
          <div className="space-y-6">
            <CardLayout title="网络配置">
              <div className="space-y-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">
                    网桥设备
                  </label>
                  <select
                    value={settings.networkBridge}
                    onChange={e => handleSettingChange('networkBridge', e.target.value)}
                    className="w-full px-3 py-2 border border-gray-300 rounded-md"
                  >
                    <option value="br0">br0</option>
                    <option value="br1">br1</option>
                    <option value="virbr0">virbr0</option>
                  </select>
                </div>
                <label className="flex items-center gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.enableNetworkOptimization}
                    onChange={e => handleSettingChange('enableNetworkOptimization', e.target.checked)}
                    className="w-4 h-4"
                  />
                  <span className="text-sm font-medium text-gray-700">
                    启用网络性能优化
                  </span>
                </label>
              </div>
            </CardLayout>

            <CardLayout title="远程管理">
              <div className="space-y-4">
                <label className="flex items-center gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={settings.enableRemoteManagement}
                    onChange={e => handleSettingChange('enableRemoteManagement', e.target.checked)}
                    className="w-4 h-4"
                  />
                  <span className="text-sm font-medium text-gray-700">
                    启用远程管理
                  </span>
                </label>
                {settings.enableRemoteManagement && (
                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">
                      远程端口
                    </label>
                    <input
                      type="number"
                      value={settings.remotePort}
                      onChange={e => handleSettingChange('remotePort', parseInt(e.target.value))}
                      className="w-full px-3 py-2 border border-gray-300 rounded-md"
                    />
                  </div>
                )}
              </div>
            </CardLayout>
          </div>
        );

      case 'advanced':
        return (
          <div className="space-y-6">
            <CardLayout title="存储配置">
              <div className="space-y-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">
                    存储池
                  </label>
                  <select
                    value={settings.storagePool}
                    onChange={e => handleSettingChange('storagePool', e.target.value)}
                    className="w-full px-3 py-2 border border-gray-300 rounded-md"
                  >
                    <option value="default">default</option>
                    <option value="fast">fast (SSD)</option>
                    <option value="archive">archive</option>
                  </select>
                </div>
              </div>
            </CardLayout>

            <CardLayout title="日志配置">
              <div className="space-y-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-1">
                    日志级别
                  </label>
                  <select
                    value={settings.logLevel}
                    onChange={e => handleSettingChange('logLevel', e.target.value)}
                    className="w-full px-3 py-2 border border-gray-300 rounded-md"
                  >
                    <option value="debug">Debug (详细)</option>
                    <option value="info">Info (信息)</option>
                    <option value="warn">Warning (警告)</option>
                    <option value="error">Error (错误)</option>
                  </select>
                </div>
              </div>
            </CardLayout>

            <CardLayout title="诊断工具">
              <div className="space-y-2">
                <Button variant="outline" className="w-full">
                  🔍 系统诊断
                </Button>
                <Button variant="outline" className="w-full">
                  📊 生成报告
                </Button>
                <Button variant="outline" className="w-full">
                  🧹 清理缓存
                </Button>
              </div>
            </CardLayout>
          </div>
        );

      default:
        return null;
    }
  };

  return (
    <div className="grid grid-cols-4 gap-6">
      {/* 左侧菜单 */}
      <div className="col-span-1">
        <div className="bg-white rounded-lg border border-gray-200 overflow-hidden sticky top-6">
          {sections.map(section => (
            <button
              key={section.id}
              onClick={() => setActiveSection(section.id)}
              className={`w-full text-left px-4 py-3 border-b border-gray-200 last:border-b-0 transition-colors ${
                activeSection === section.id
                  ? 'bg-blue-50 text-blue-600 font-medium'
                  : 'text-gray-700 hover:bg-gray-50'
              }`}
            >
              <span className="text-lg mr-2">{section.icon}</span>
              {section.label}
            </button>
          ))}
        </div>
      </div>

      {/* 右侧内容 */}
      <div className="col-span-3">
        <div className="space-y-6">
          {renderContent()}

          {/* 操作按钮 */}
          <div className="flex gap-2 justify-end sticky bottom-6 bg-white p-4 rounded-lg border border-gray-200">
            <Button variant="outline" onClick={handleReset}>
              ↺ 恢复默认
            </Button>
            <Button variant="primary" onClick={handleSave}>
              💾 保存设置
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
};
