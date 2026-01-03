//! # Core Traits - VM Component Foundation
//!
//! This module defines the foundational traits for all VM components,
//! establishing a consistent lifecycle and management interface.

use serde::{Deserialize, Serialize};

use super::{ComponentStatus, StateEventCallback, SubscriptionId};
use crate::VmError;

/// Base trait for all VM components, defining lifecycle management.
///
/// All components in the VM system (memory managers, execution engines,
/// devices, etc.) must implement this trait to ensure consistent
/// initialization, startup, and shutdown behavior.
///
/// # Type Parameters
///
/// * `Config` - Configuration type for component initialization
/// * `Error` - Error type that can be returned from operations
///
/// # Lifecycle
///
/// Components follow this lifecycle:
/// 1. **Uninitialized** → `init()` → **Initialized**
/// 2. **Initialized** → `start()` → **Running**
/// 3. **Running** → `stop()` → **Stopped**
///
/// # Examples
///
/// ```
/// use vm_core::interface::VmComponent;
/// use vm_core::interface::ComponentStatus;
/// use vm_core::VmError;
///
/// struct MyComponent {
///     status: ComponentStatus,
/// }
///
/// impl VmComponent for MyComponent {
///     type Config = String;
///     type Error = VmError;
///
///     fn init(config: Self::Config) -> Result<Self, Self::Error> {
///         Ok(Self { status: ComponentStatus::Initialized })
///     }
///
///     fn start(&mut self) -> Result<(), Self::Error> {
///         self.status = ComponentStatus::Running;
///         Ok(())
///     }
///
///     fn stop(&mut self) -> Result<(), Self::Error> {
///         self.status = ComponentStatus::Stopped;
///         Ok(())
///     }
///
///     fn status(&self) -> ComponentStatus {
///         self.status
///     }
///
///     fn name(&self) -> &str {
///         "MyComponent"
///     }
/// }
/// ```
pub trait VmComponent {
    /// Configuration type for this component
    type Config;

    /// Error type that can be returned from operations
    type Error;

    /// Initialize the component with the given configuration.
    ///
    /// This method is called once during component creation to set up
    /// internal state and resources. After successful initialization,
    /// the component should be in the `Initialized` state.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration data for component initialization
    ///
    /// # Returns
    ///
    /// Returns the initialized component or an error if initialization fails.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The configuration is invalid
    /// - Required resources cannot be allocated
    /// - Internal setup fails
    fn init(config: Self::Config) -> Result<Self, Self::Error>
    where
        Self: Sized;

    /// Start the component.
    ///
    /// Transitions the component from `Initialized` or `Stopped` state
    /// to `Running` state. This should activate the component's main
    /// functionality.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the component started successfully.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The component is already running
    /// - Required resources are unavailable
    /// - The component fails to start its internal operations
    fn start(&mut self) -> Result<(), Self::Error>;

    /// Stop the component.
    ///
    /// Transitions the component from `Running` state to `Stopped` state.
    /// This should gracefully shut down the component's operations while
    /// preserving its state for potential restart.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the component stopped successfully.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The component is not running
    /// - Clean shutdown fails
    /// - Resources cannot be properly released
    fn stop(&mut self) -> Result<(), Self::Error>;

    /// Get the current status of the component.
    ///
    /// Returns the current lifecycle state of the component, which can be
    /// used to check if the component is ready for operations.
    ///
    /// # Returns
    ///
    /// Current component status
    fn status(&self) -> ComponentStatus;

    /// Get the name of the component.
    ///
    /// Returns a unique identifier for this component type, useful for
    /// logging, debugging, and component management.
    ///
    /// # Returns
    ///
    /// Component name as a string slice
    fn name(&self) -> &str;
}

/// Trait for components that support runtime configuration.
///
/// This trait enables dynamic reconfiguration of VM components during
/// runtime, allowing settings to be updated without requiring a restart.
///
/// # Type Parameters
///
/// * `Config` - Configuration type that must be serializable and cloneable
///
/// # Examples
///
/// ```
/// use vm_core::interface::Configurable;
/// use vm_core::VmError;
///
/// struct MyComponent {
///     config: MyConfig,
/// }
///
/// #[derive(Clone, Debug)]
/// struct MyConfig {
///     buffer_size: usize,
///     timeout_ms: u64,
/// }
///
/// impl Configurable for MyComponent {
///     type Config = MyConfig;
///
///     fn update_config(&mut self, config: &Self::Config) -> Result<(), VmError> {
///         self.config = config.clone();
///         Ok(())
///     }
///
///     fn get_config(&self) -> &Self::Config {
///         &self.config
///     }
///
///     fn validate_config(config: &Self::Config) -> Result<(), VmError> {
///         if config.buffer_size == 0 {
///             return Err(VmError::Generic {
///                 message: "buffer_size must be > 0".to_string()
///             });
///         }
///         Ok(())
///     }
/// }
/// ```
pub trait Configurable {
    /// Configuration type for this component
    type Config;

    /// Update the component's configuration at runtime.
    ///
    /// This method applies a new configuration to the component. The
    /// implementation should validate the configuration and apply changes
    /// atomically if possible.
    ///
    /// # Arguments
    ///
    /// * `config` - New configuration to apply
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the configuration was successfully applied.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The configuration is invalid
    /// - The component cannot apply the new configuration in its current state
    /// - Applying the configuration would require a restart
    fn update_config(&mut self, config: &Self::Config) -> Result<(), VmError>;

    /// Get the current configuration of the component.
    ///
    /// Returns a reference to the component's active configuration,
    /// allowing inspection of current settings.
    ///
    /// # Returns
    ///
    /// Reference to the current configuration
    fn get_config(&self) -> &Self::Config;

    /// Validate a configuration before applying it.
    ///
    /// This method checks if a configuration is valid without modifying
    /// the component's state. Useful for configuration validation UIs
    /// and pre-flight checks.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration to validate
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the configuration is valid.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Required fields are missing
    /// - Values are out of valid ranges
    /// - Fields have incompatible values
    fn validate_config(config: &Self::Config) -> Result<(), VmError>;
}

/// Trait for components that support state observation and event notifications.
///
/// This trait implements the observer pattern, allowing external code to
/// subscribe to state changes and events from VM components. This is useful
/// for monitoring, debugging, and reactive programming.
///
/// # Type Parameters
///
/// * `State` - Type representing the component's state
/// * `Event` - Type representing events that can be emitted
///
/// # Examples
///
/// ```
/// use vm_core::interface::{Observable, StateEventCallback, SubscriptionId};
/// use vm_core::VmError;
///
/// struct MyObservable {
///     state: MyState,
///     subscribers: std::collections::HashMap<SubscriptionId, StateEventCallback<MyState, MyEvent>>,
/// }
///
/// #[derive(Clone, Debug)]
/// struct MyState { value: u64 }
///
/// #[derive(Clone, Debug)]
/// enum MyEvent { ValueChanged(u64) }
///
/// impl Observable for MyObservable {
///     type State = MyState;
///     type Event = MyEvent;
///
///     fn get_state(&self) -> &Self::State {
///         &self.state
///     }
///
///     fn subscribe(
///         &mut self,
///         callback: StateEventCallback<Self::State, Self::Event>,
///     ) -> SubscriptionId {
///         let id = 1;
///         self.subscribers.insert(id, callback);
///         id
///     }
///
///     fn unsubscribe(&mut self, id: SubscriptionId) -> Result<(), VmError> {
///         self.subscribers.remove(&id);
///         Ok(())
///     }
/// }
/// ```
pub trait Observable {
    /// State type for this component
    type State;

    /// Event type that can be emitted
    type Event;

    /// Get the current state of the component.
    ///
    /// Returns an immutable reference to the component's current state,
    /// allowing inspection without modification.
    ///
    /// # Returns
    ///
    /// Reference to the current state
    fn get_state(&self) -> &Self::State;

    /// Subscribe to state changes and events.
    ///
    /// Registers a callback that will be invoked whenever the component's
    /// state changes or it emits an event. The callback receives both the
    /// new state and the event that triggered the notification.
    ///
    /// # Arguments
    ///
    /// * `callback` - Function to call when state changes or events occur
    ///
    /// # Returns
    ///
    /// Unique subscription ID that can be used to unsubscribe
    ///
    /// # Examples
    ///
    /// ```
    /// # use vm_core::interface::{Observable, StateEventCallback};
    /// # struct MyComponent;
    /// # impl Observable for MyComponent {
    /// #     type State = u64;
    /// #     type Event = String;
    /// #     fn get_state(&self) -> &Self::State { &0 }
    /// #     fn subscribe(&mut self, _: StateEventCallback<Self::State, Self::Event>) -> u64 { 0 }
    /// #     fn unsubscribe(&mut self, _: u64) -> Result<(), vm_core::VmError> { Ok(()) }
    /// # }
    /// let mut component = MyComponent;
    /// let callback: StateEventCallback<u64, String> = Box::new(|state, event| {
    ///     println!("State: {:?}, Event: {:?}", state, event);
    /// });
    /// let id = component.subscribe(callback);
    /// ```
    fn subscribe(
        &mut self,
        callback: StateEventCallback<Self::State, Self::Event>,
    ) -> SubscriptionId;

    /// Unsubscribe from state changes and events.
    ///
    /// Removes a previously registered callback. The subscription ID
    /// must have been returned from a prior call to `subscribe()`.
    ///
    /// # Arguments
    ///
    /// * `id` - Subscription ID returned from `subscribe()`
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the subscription was removed.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The subscription ID is invalid
    /// - The subscription was already removed
    fn unsubscribe(&mut self, id: SubscriptionId) -> Result<(), VmError>;
}

/// Base component state representation.
///
/// Contains common state information shared by all VM components,
/// including status, timing, and error information.
///
/// # Fields
///
/// * `name` - Component name
/// * `status` - Current lifecycle status
/// * `start_time` - When the component was started (if running)
/// * `last_error` - Most recent error message (if any)
///
/// # Examples
///
/// ```
/// use vm_core::interface::{ComponentState, ComponentStatus};
/// use serde_json;
///
/// let state = ComponentState {
///     name: "MyComponent".to_string(),
///     status: ComponentStatus::Running,
///     start_time: Some(std::time::SystemTime::now()),
///     last_error: None,
/// };
///
/// // Can be serialized for monitoring
/// let json = serde_json::to_string(&state).unwrap();
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentState {
    /// Component name
    pub name: String,

    /// Current lifecycle status
    pub status: ComponentStatus,

    /// When the component was started (if running)
    pub start_time: Option<std::time::SystemTime>,

    /// Most recent error message (if any)
    pub last_error: Option<String>,
}

impl Default for ComponentState {
    fn default() -> Self {
        Self {
            name: String::new(),
            status: ComponentStatus::Uninitialized,
            start_time: None,
            last_error: None,
        }
    }
}

/// Manager for multiple VM components.
///
/// Provides centralized lifecycle management for collections of VM components,
/// supporting registration, lookup, and bulk start/stop operations.
///
/// # Examples
///
/// ```
/// use vm_core::interface::ComponentManager;
///
/// let mut manager = ComponentManager::new();
/// // Components can be registered and managed collectively
/// manager.start_all().unwrap();
/// ```
pub struct ComponentManager {
    /// Map of component name to component instance
    components: std::collections::HashMap<
        String,
        Box<dyn VmComponent<Config = serde_json::Value, Error = VmError>>,
    >,
}

impl Default for ComponentManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ComponentManager {
    /// Create a new empty component manager.
    ///
    /// # Returns
    ///
    /// A new ComponentManager with no registered components
    ///
    /// # Examples
    ///
    /// ```
    /// use vm_core::interface::ComponentManager;
    ///
    /// let manager = ComponentManager::new();
    /// assert_eq!(manager.list_components().len(), 0);
    /// ```
    pub fn new() -> Self {
        Self {
            components: std::collections::HashMap::new(),
        }
    }

    /// Register a component with the manager.
    ///
    /// # Type Parameters
    ///
    /// * `C` - Component type that implements VmComponent
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for the component
    /// * `component` - Component instance to register
    ///
    /// # Examples
    ///
    /// ```
    /// # use vm_core::interface::{ComponentManager, VmComponent, ComponentStatus};
    /// # use vm_core::VmError;
    /// # struct MyComponent;
    /// # impl VmComponent for MyComponent {
    /// #   type Config = serde_json::Value;
    /// #   type Error = VmError;
    /// #   fn init(_: Self::Config) -> Result<Self, Self::Error> { Ok(Self {}) }
    /// #   fn start(&mut self) -> Result<(), Self::Error> { Ok(()) }
    /// #   fn stop(&mut self) -> Result<(), Self::Error> { Ok(()) }
    /// #   fn status(&self) -> ComponentStatus { ComponentStatus::Running }
    /// #   fn name(&self) -> &str { "MyComponent" }
    /// # }
    /// let mut manager = ComponentManager::new();
    /// manager.register_component("my_component".to_string(), MyComponent {});
    /// ```
    pub fn register_component<C>(&mut self, name: String, component: C)
    where
        C: VmComponent<Config = serde_json::Value, Error = VmError> + 'static,
    {
        self.components.insert(name, Box::new(component));
    }

    /// Get a reference to a component by name.
    ///
    /// # Arguments
    ///
    /// * `name` - Component name
    ///
    /// # Returns
    ///
    /// `Some(&component)` if found, `None` otherwise
    pub fn get_component(
        &self,
        name: &str,
    ) -> Option<&dyn VmComponent<Config = serde_json::Value, Error = VmError>> {
        self.components.get(name).map(|v| v.as_ref())
    }

    /// Get a mutable reference to a component by name.
    ///
    /// # Arguments
    ///
    /// * `name` - Component name
    ///
    /// # Returns
    ///
    /// `Some(&mut component)` if found, `None` otherwise
    pub fn get_component_mut(
        &mut self,
        name: &str,
    ) -> Option<&mut Box<dyn VmComponent<Config = serde_json::Value, Error = VmError>>> {
        self.components.get_mut(name)
    }

    /// List all registered component names.
    ///
    /// # Returns
    ///
    /// Vector of component names
    pub fn list_components(&self) -> Vec<&str> {
        self.components.keys().map(|s| s.as_str()).collect()
    }

    /// Start all registered components.
    ///
    /// Attempts to start all components in order. If any component fails
    /// to start, the operation is aborted and an error is returned.
    /// Components that were already started remain running.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if all components started successfully.
    ///
    /// # Errors
    ///
    /// Returns an error if any component fails to start
    pub fn start_all(&mut self) -> Result<(), VmError> {
        for (name, component) in &mut self.components {
            component.start().map_err(|e| {
                VmError::Core(crate::CoreError::Internal {
                    message: format!("Failed to start component '{}': {:?}", name, e),
                    module: "ComponentManager".to_string(),
                })
            })?;
        }
        Ok(())
    }

    /// Stop all registered components.
    ///
    /// Attempts to stop all components in order. If any component fails
    /// to stop, the operation continues but an error is returned.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if all components stopped successfully.
    ///
    /// # Errors
    ///
    /// Returns an error if any component fails to stop
    pub fn stop_all(&mut self) -> Result<(), VmError> {
        for (name, component) in &mut self.components {
            component.stop().map_err(|e| {
                VmError::Core(crate::CoreError::Internal {
                    message: format!("Failed to stop component '{}': {:?}", name, e),
                    module: "ComponentManager".to_string(),
                })
            })?;
        }
        Ok(())
    }
}
