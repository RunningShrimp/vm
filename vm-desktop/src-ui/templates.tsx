/**
 * 模板组件库 - 页面模板和布局
 * 模仿 VMware Workstation 的整体布局结构
 */

import React from 'react';
import { Button } from './atoms';

/** 主应用布局模板 */
export interface MainLayoutProps {
  children: React.ReactNode;
  sidebar?: React.ReactNode;
  header?: React.ReactNode;
  footer?: React.ReactNode;
  sidebarOpen?: boolean;
}

export const MainLayout: React.FC<MainLayoutProps> = ({
  children,
  sidebar,
  header,
  footer,
  sidebarOpen = true,
}) => {
  return (
    <div className="flex h-screen bg-gray-100">
      {/* 左侧导航栏 */}
      {sidebarOpen && sidebar && (
        <aside className="w-64 bg-gray-900 text-white flex flex-col shadow-lg">
          {sidebar}
        </aside>
      )}

      {/* 主内容区 */}
      <div className="flex-1 flex flex-col">
        {/* 顶部栏 */}
        {header && (
          <header className="bg-white border-b border-gray-200 shadow-sm sticky top-0 z-10">
            {header}
          </header>
        )}

        {/* 主体内容 */}
        <main className="flex-1 overflow-auto">
          <div className="p-6">
            {children}
          </div>
        </main>

        {/* 底部栏 */}
        {footer && (
          <footer className="bg-white border-t border-gray-200 px-6 py-3 text-sm text-gray-600">
            {footer}
          </footer>
        )}
      </div>
    </div>
  );
};

/** 导航栏模板 */
export interface SidebarProps {
  items: SidebarItem[];
  activeItem?: string;
  onSelect?: (item: string) => void;
  logo?: React.ReactNode;
}

export interface SidebarItem {
  id: string;
  label: string;
  icon: React.ReactNode;
  badge?: string;
  disabled?: boolean;
}

export const Sidebar: React.FC<SidebarProps> = ({
  items,
  activeItem,
  onSelect,
  logo,
}) => {
  return (
    <div className="flex flex-col h-full">
      {/* Logo 区 */}
      {logo && (
        <div className="px-6 py-6 border-b border-gray-700">
          {logo}
        </div>
      )}

      {/* 菜单项 */}
      <nav className="flex-1 overflow-y-auto px-3 py-4 space-y-2">
        {items.map((item) => (
          <button
            key={item.id}
            onClick={() => onSelect?.(item.id)}
            disabled={item.disabled}
            className={`w-full flex items-center gap-3 px-4 py-3 rounded-lg transition-colors ${
              activeItem === item.id
                ? 'bg-blue-600 text-white'
                : 'text-gray-300 hover:bg-gray-800 hover:text-white'
            } ${item.disabled ? 'opacity-50 cursor-not-allowed' : ''}`}
          >
            <span className="text-xl">{item.icon}</span>
            <span className="flex-1 text-left font-medium">{item.label}</span>
            {item.badge && (
              <span className="bg-red-600 text-white text-xs px-2 py-1 rounded-full">
                {item.badge}
              </span>
            )}
          </button>
        ))}
      </nav>

      {/* 底部操作区 */}
      <div className="px-3 py-4 border-t border-gray-700 space-y-2">
        <button className="w-full flex items-center gap-3 px-4 py-3 rounded-lg text-gray-300 hover:bg-gray-800 hover:text-white transition-colors">
          <span className="text-xl">⚙️</span>
          <span className="flex-1 text-left font-medium">设置</span>
        </button>
        <button className="w-full flex items-center gap-3 px-4 py-3 rounded-lg text-gray-300 hover:bg-gray-800 hover:text-white transition-colors">
          <span className="text-xl">👤</span>
          <span className="flex-1 text-left font-medium">账户</span>
        </button>
      </div>
    </div>
  );
};

/** 顶部栏模板 */
export interface TopBarProps {
  title?: string;
  breadcrumbs?: Breadcrumb[];
  actions?: React.ReactNode;
  searchBar?: boolean;
}

export interface Breadcrumb {
  label: string;
  onClick?: () => void;
}

export const TopBar: React.FC<TopBarProps> = ({
  title,
  breadcrumbs,
  actions,
  searchBar = false,
}) => {
  return (
    <div className="px-6 py-4 flex items-center justify-between gap-4">
      <div className="flex-1 flex items-center gap-4">
        {/* 标题和面包屑 */}
        <div>
          {breadcrumbs && breadcrumbs.length > 0 && (
            <nav className="text-sm text-gray-600 mb-1">
              {breadcrumbs.map((crumb, index) => (
                <React.Fragment key={index}>
                  <button
                    onClick={crumb.onClick}
                    className="hover:text-blue-600"
                  >
                    {crumb.label}
                  </button>
                  {index < breadcrumbs.length - 1 && <span className="mx-2">/</span>}
                </React.Fragment>
              ))}
            </nav>
          )}
          {title && <h1 className="text-xl font-semibold text-gray-900">{title}</h1>}
        </div>

        {/* 搜索栏 */}
        {searchBar && (
          <div className="flex-1 max-w-md">
            <input
              type="text"
              placeholder="搜索..."
              className="w-full px-4 py-2 rounded-lg border border-gray-300 focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
          </div>
        )}
      </div>

      {/* 右侧操作按钮 */}
      {actions && <div className="flex items-center gap-2">{actions}</div>}
    </div>
  );
};

/** 内容卡片模板 */
export interface CardLayoutProps {
  title?: string;
  subtitle?: string;
  children: React.ReactNode;
  actions?: React.ReactNode;
  className?: string;
}

export const CardLayout: React.FC<CardLayoutProps> = ({
  title,
  subtitle,
  children,
  actions,
  className = '',
}) => {
  return (
    <div className={`bg-white rounded-lg shadow-md overflow-hidden ${className}`}>
      {(title || subtitle || actions) && (
        <div className="border-b border-gray-200 px-6 py-4 flex justify-between items-start">
          <div>
            {title && <h3 className="text-lg font-semibold text-gray-900">{title}</h3>}
            {subtitle && <p className="text-sm text-gray-600 mt-1">{subtitle}</p>}
          </div>
          {actions && <div className="flex gap-2">{actions}</div>}
        </div>
      )}
      <div className="p-6">{children}</div>
    </div>
  );
};

/** 表单模板 */
export interface FormLayoutProps {
  title?: string;
  subtitle?: string;
  onSubmit?: (e: React.FormEvent) => void;
  children: React.ReactNode;
  submitLabel?: string;
  cancelLabel?: string;
  onCancel?: () => void;
  loading?: boolean;
}

export const FormLayout: React.FC<FormLayoutProps> = ({
  title,
  subtitle,
  onSubmit,
  children,
  submitLabel = '提交',
  cancelLabel = '取消',
  onCancel,
  loading = false,
}) => {
  return (
    <form onSubmit={onSubmit} className="space-y-6">
      {title && (
        <div>
          <h2 className="text-2xl font-bold text-gray-900">{title}</h2>
          {subtitle && <p className="text-gray-600 mt-1">{subtitle}</p>}
        </div>
      )}

      <div className="space-y-4">{children}</div>

      <div className="flex gap-4 pt-6 border-t border-gray-200">
        <Button type="submit" variant="primary" disabled={loading}>
          {loading ? '处理中...' : submitLabel}
        </Button>
        {onCancel && (
          <Button type="button" variant="outline" onClick={onCancel}>
            {cancelLabel}
          </Button>
        )}
      </div>
    </form>
  );
};

/** 两列布局 */
export interface TwoColumnLayoutProps {
  left: React.ReactNode;
  right: React.ReactNode;
  leftWidth?: string;
}

export const TwoColumnLayout: React.FC<TwoColumnLayoutProps> = ({
  left,
  right,
  leftWidth = '1/3',
}) => {
  return (
    <div className="grid grid-cols-3 gap-6">
      <div className={`col-span-${parseInt(leftWidth.split('/')[0])}`}>
        {left}
      </div>
      <div className={`col-span-${parseInt(leftWidth.split('/')[1]) - parseInt(leftWidth.split('/')[0])}`}>
        {right}
      </div>
    </div>
  );
};

/** 标签页布局 */
export interface TabLayout {
  label: string;
  value: string;
  icon?: React.ReactNode;
  content: React.ReactNode;
}

export interface TabsLayoutProps {
  tabs: TabLayout[];
  activeTab?: string;
  onChange?: (tab: string) => void;
}

export const TabsLayout: React.FC<TabsLayoutProps> = ({
  tabs,
  activeTab = tabs[0]?.value,
  onChange,
}) => {
  return (
    <div className="space-y-4">
      <div className="flex gap-2 border-b border-gray-200">
        {tabs.map((tab) => (
          <button
            key={tab.value}
            onClick={() => onChange?.(tab.value)}
            className={`px-4 py-3 font-medium border-b-2 transition-colors flex items-center gap-2 ${
              activeTab === tab.value
                ? 'border-blue-600 text-blue-600'
                : 'border-transparent text-gray-600 hover:text-gray-900'
            }`}
          >
            {tab.icon && <span>{tab.icon}</span>}
            {tab.label}
          </button>
        ))}
      </div>
      <div className="py-4">
        {tabs.find((t) => t.value === activeTab)?.content}
      </div>
    </div>
  );
};

/** 模态对话框布局 */
export interface ModalLayoutProps {
  open?: boolean;
  title?: string;
  subtitle?: string;
  children: React.ReactNode;
  onClose?: () => void;
  actions?: React.ReactNode;
  size?: 'sm' | 'md' | 'lg' | 'xl';
}

export const ModalLayout: React.FC<ModalLayoutProps> = ({
  open = true,
  title,
  subtitle,
  children,
  onClose,
  actions,
  size = 'md',
}) => {
  if (!open) return null;

  const sizeClasses = {
    sm: 'max-w-sm',
    md: 'max-w-md',
    lg: 'max-w-lg',
    xl: 'max-w-xl',
  };

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className={`bg-white rounded-lg shadow-xl ${sizeClasses[size]}`}>
        {/* Header */}
        {(title || subtitle) && (
          <div className="border-b border-gray-200 px-6 py-4">
            {title && <h2 className="text-lg font-semibold text-gray-900">{title}</h2>}
            {subtitle && <p className="text-sm text-gray-600 mt-1">{subtitle}</p>}
          </div>
        )}

        {/* Content */}
        <div className="px-6 py-4">{children}</div>

        {/* Footer */}
        {actions && (
          <div className="border-t border-gray-200 px-6 py-4 flex justify-end gap-2">
            {actions}
          </div>
        )}
      </div>
    </div>
  );
};

/** 网格布局 */
export interface GridLayoutProps {
  children: React.ReactNode;
  columns?: number;
  gap?: 'sm' | 'md' | 'lg';
}

export const GridLayout: React.FC<GridLayoutProps> = ({
  children,
  columns = 3,
  gap = 'md',
}) => {
  const gapClasses = {
    sm: 'gap-2',
    md: 'gap-4',
    lg: 'gap-6',
  };

  const colsClass = `grid-cols-${columns}`;

  return (
    <div className={`grid ${colsClass} ${gapClasses[gap]}`}>
      {children}
    </div>
  );
};

/** 堆栈布局 */
export interface StackLayoutProps {
  children: React.ReactNode;
  direction?: 'vertical' | 'horizontal';
  spacing?: 'xs' | 'sm' | 'md' | 'lg';
  align?: 'start' | 'center' | 'end' | 'stretch';
}

export const StackLayout: React.FC<StackLayoutProps> = ({
  children,
  direction = 'vertical',
  spacing = 'md',
  align = 'stretch',
}) => {
  const spacingClasses = {
    xs: 'gap-2',
    sm: 'gap-3',
    md: 'gap-4',
    lg: 'gap-6',
  };

  const alignClasses = {
    start: 'items-start',
    center: 'items-center',
    end: 'items-end',
    stretch: 'items-stretch',
  };

  const flexClass = direction === 'vertical' ? 'flex-col' : 'flex-row';

  return (
    <div className={`flex ${flexClass} ${spacingClasses[spacing]} ${alignClasses[align]}`}>
      {children}
    </div>
  );
};

/** 列表项布局 */
export interface ListItemLayoutProps {
  icon?: React.ReactNode;
  title?: string;
  subtitle?: string;
  value?: string;
  actions?: React.ReactNode;
  onClick?: () => void;
}

export const ListItemLayout: React.FC<ListItemLayoutProps> = ({
  icon,
  title,
  subtitle,
  value,
  actions,
  onClick,
}) => {
  return (
    <div
      onClick={onClick}
      className={`flex items-center gap-4 p-4 bg-white rounded-lg border border-gray-200 hover:shadow-md transition-shadow ${
        onClick ? 'cursor-pointer' : ''
      }`}
    >
      {icon && <div className="text-2xl flex-shrink-0">{icon}</div>}

      <div className="flex-1">
        {title && <div className="font-medium text-gray-900">{title}</div>}
        {subtitle && <div className="text-sm text-gray-600">{subtitle}</div>}
      </div>

      {value && <div className="text-lg font-semibold text-gray-900">{value}</div>}

      {actions && <div className="flex gap-2">{actions}</div>}
    </div>
  );
};
