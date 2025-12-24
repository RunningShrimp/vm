//! 性能监控和分析工具
//!
//! 提供采样、火焰图和时间线功能

use std::collections::{HashMap, BTreeMap};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

/// 采样配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilerConfig {
    /// 采样间隔（纳秒）
    pub sample_interval_ns: u64,
    /// 最大采样次数
    pub max_samples: usize,
    /// 是否启用火焰图
    pub enable_flamegraph: bool,
    /// 是否启用时间线
    pub enable_timeline: bool,
    /// 是否跟踪内存分配
    pub track_memory: bool,
}

impl Default for ProfilerConfig {
    fn default() -> Self {
        Self {
            sample_interval_ns: 1_000_000, // 1ms
            max_samples: 100_000,
            enable_flamegraph: true,
            enable_timeline: true,
            track_memory: true,
        }
    }
}

/// 采样点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplePoint {
    /// 时间戳（纳秒）
    pub timestamp_ns: u64,
    /// 函数/操作名称
    pub name: String,
    /// 持续时间（纳秒）
    pub duration_ns: u64,
    /// 调用栈深度
    pub depth: u32,
    /// 内存使用量（字节）
    pub memory_usage: Option<u64>,
    /// CPU 使用率（百分比）
    pub cpu_usage: Option<f64>,
    /// 自定义数据
    pub metadata: HashMap<String, String>,
}

/// 调用栈帧
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackFrame {
    /// 函数/操作名称
    pub name: String,
    /// 开始时间
    pub start_time: Instant,
    /// 子帧
    pub children: Vec<StackFrame>,
}

impl StackFrame {
    pub fn new(name: String) -> Self {
        Self {
            name,
            start_time: Instant::now(),
            children: Vec::new(),
        }
    }

    pub fn duration(&self) -> Duration {
        let end = self.children
            .iter()
            .map(|c| c.start_time)
            .max()
            .unwrap_or(self.start_time);

        end.saturating_duration_since(self.start_time)
    }
}

/// 性能分析器
pub struct Profiler {
    config: ProfilerConfig,
    samples: Vec<SamplePoint>,
    call_stack: Vec<StackFrame>,
    current_frame: Option<StackFrame>,
    stats: ProfilerStats,
    start_time: Instant,
}

impl Profiler {
    pub fn new(config: ProfilerConfig) -> Self {
        Self {
            config,
            samples: Vec::new(),
            call_stack: Vec::new(),
            current_frame: None,
            stats: ProfilerStats::default(),
            start_time: Instant::now(),
        }
    }

    /// 开始记录一个函数/操作
    pub fn enter(&mut self, name: String) {
        let frame = StackFrame::new(name.clone());
        self.call_stack.push(frame);
        self.current_frame = Some(frame);
        self.stats.total_entries += 1;
    }

    /// 结束记录当前函数/操作
    pub fn exit(&mut self, name: &str) {
        if let Some(mut frame) = self.current_frame.take() {
            if frame.name != name {
                eprintln!("Warning: Mismatched exit: {} != {}", name, frame.name);
            }

            let duration = frame.duration();
            let sample = SamplePoint {
                timestamp_ns: self.start_time.elapsed().as_nanos() as u64,
                name: frame.name.clone(),
                duration_ns: duration.as_nanos() as u64,
                depth: self.call_stack.len() as u32,
                memory_usage: None,
                cpu_usage: None,
                metadata: HashMap::new(),
            };

            self.samples.push(sample);
            self.stats.total_samples += 1;
            self.stats.total_duration_ns += duration.as_nanos() as u64;
        }

        if !self.call_stack.is_empty() {
            self.current_frame = self.call_stack.pop();
        }
    }

    /// 添加元数据
    pub fn add_metadata(&mut self, key: String, value: String) {
        if let Some(sample) = self.samples.last_mut() {
            sample.metadata.insert(key, value);
        }
    }

    /// 获取所有样本
    pub fn samples(&self) -> &[SamplePoint] {
        &self.samples
    }

    /// 获取统计信息
    pub fn stats(&self) -> &ProfilerStats {
        &self.stats
    }

    /// 生成火焰图数据
    pub fn generate_flamegraph(&self) -> FlamegraphData {
        let mut root = FlamegraphNode::new("root".to_string());

        for sample in &self.samples {
            let mut current = &mut root;
            for _ in 0..sample.depth {
                if current.children.is_empty() {
                    current.children.push(FlamegraphNode::new(sample.name.clone()));
                }
                current = current.children.last_mut().unwrap();
            }
            current.value += sample.duration_ns;
        }

        FlamegraphData { root }
    }

    /// 生成时间线数据
    pub fn generate_timeline(&self) -> TimelineData {
        let mut events = BTreeMap::new();

        for sample in &self.samples {
            let start = sample.timestamp_ns;
            let end = start + sample.duration_ns;

            events.entry(start).or_insert_with(Vec::new).push(TimelineEvent {
                name: sample.name.clone(),
                start,
                end,
                depth: sample.depth,
            });
        }

        TimelineData { events }
    }
}

/// 统计信息
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfilerStats {
    pub total_samples: u64,
    pub total_entries: u64,
    pub total_duration_ns: u64,
    pub max_depth: u32,
}

/// 火焰图节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlamegraphNode {
    pub name: String,
    pub value: u64,
    pub children: Vec<FlamegraphNode>,
}

impl FlamegraphNode {
    pub fn new(name: String) -> Self {
        Self {
            name,
            value: 0,
            children: Vec::new(),
        }
    }
}

/// 火焰图数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlamegraphData {
    pub root: FlamegraphNode,
}

impl FlamegraphData {
    /// 转换为 SVG 格式
    pub fn to_svg(&self) -> String {
        let mut svg = String::new();
        svg.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        svg.push_str("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100%\" height=\"100%\">\n");

        self.render_node(&self.root, 0, 0, 100, &mut svg);

        svg.push_str("</svg>");
        svg
    }

    fn render_node(&self, node: &FlamegraphNode, x: f64, y: f64, width: f64, svg: &mut String) {
        let height = 30.0;
        let color = self.color_for_name(&node.name);

        svg.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\" stroke=\"white\" stroke-width=\"1\"/>\n",
            x, y, width, height, color
        ));

        svg.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" font-size=\"12\" fill=\"black\" text-anchor=\"middle\">{}</text>\n",
            x + width / 2.0, y + height / 2.0 + 4.0, node.name
        ));

        let mut child_x = x;
        let child_width = if node.children.is_empty() {
            0.0
        } else {
            width / node.children.len() as f64
        };

        for child in &node.children {
            self.render_node(child, child_x, y + height, child_width, svg);
            child_x += child_width;
        }
    }

    fn color_for_name(&self, name: &str) -> String {
        let hash = name.bytes().fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
        let r = (hash & 0xFF0000) >> 16;
        let g = (hash & 0x00FF00) >> 8;
        let b = hash & 0x0000FF;
        format!("#{:02X}{:02X}{:02X}", r, g, b)
    }
}

/// 时间线事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub name: String,
    pub start: u64,
    pub end: u64,
    pub depth: u32,
}

/// 时间线数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineData {
    pub events: BTreeMap<u64, Vec<TimelineEvent>>,
}

impl TimelineData {
    /// 转换为 JSON 格式
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// 线程安全的性能分析器
pub struct ThreadSafeProfiler {
    inner: Arc<Mutex<Profiler>>,
}

impl ThreadSafeProfiler {
    pub fn new(config: ProfilerConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Profiler::new(config))),
        }
    }

    pub fn enter(&self, name: String) {
        let mut inner = self.inner.lock().unwrap();
        inner.enter(name);
    }

    pub fn exit(&self, name: &str) {
        let mut inner = self.inner.lock().unwrap();
        inner.exit(name);
    }

    pub fn samples(&self) -> Vec<SamplePoint> {
        let inner = self.inner.lock().unwrap();
        inner.samples().to_vec()
    }

    pub fn generate_flamegraph(&self) -> FlamegraphData {
        let inner = self.inner.lock().unwrap();
        inner.generate_flamegraph()
    }

    pub fn generate_timeline(&self) -> TimelineData {
        let inner = self.inner.lock().unwrap();
        inner.generate_timeline()
    }
}

/// 性能分析器作用域
pub struct ProfilerScope<'a> {
    profiler: &'a mut Profiler,
    name: String,
}

impl<'a> ProfilerScope<'a> {
    pub fn new(profiler: &'a mut Profiler, name: String) -> Self {
        profiler.enter(name.clone());
        Self { profiler, name }
    }
}

impl<'a> Drop for ProfilerScope<'a> {
    fn drop(&mut self) {
        self.profiler.exit(&self.name);
    }
}

impl Profiler {
    /// 创建作用域
    pub fn scope(&mut self, name: String) -> ProfilerScope {
        ProfilerScope::new(self, name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profiler_basic() {
        let config = ProfilerConfig::default();
        let mut profiler = Profiler::new(config);

        profiler.enter("function1".to_string());
        std::thread::sleep(Duration::from_millis(10));
        profiler.exit("function1");

        assert_eq!(profiler.samples().len(), 1);
        assert_eq!(profiler.samples()[0].name, "function1");
    }

    #[test]
    fn test_profiler_scope() {
        let config = ProfilerConfig::default();
        let mut profiler = Profiler::new(config);

        {
            let _scope = profiler.scope("function1".to_string());
            std::thread::sleep(Duration::from_millis(10));
        }

        assert_eq!(profiler.samples().len(), 1);
    }

    #[test]
    fn test_flamegraph_generation() {
        let config = ProfilerConfig::default();
        let mut profiler = Profiler::new(config);

        profiler.enter("root".to_string());
        profiler.enter("child1".to_string());
        std::thread::sleep(Duration::from_millis(5));
        profiler.exit("child1");
        profiler.enter("child2".to_string());
        std::thread::sleep(Duration::from_millis(5));
        profiler.exit("child2");
        profiler.exit("root");

        let flamegraph = profiler.generate_flamegraph();
        assert_eq!(flamegraph.root.name, "root");
        assert_eq!(flamegraph.root.children.len(), 2);
    }
}
