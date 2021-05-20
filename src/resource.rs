use rayon::ThreadPool;
use rayon::ThreadPoolBuilder;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

pub enum Resource<T>
{
  Ready(T),
  Load,
  Fail(&'static str),
}

pub trait ResourceMethod<T, P: ResourceProvider<T>>
{
  /// Acquires an `Arc<Mut<Resource<T>>>`. This decouples the
  /// loading method from the resource loading. The loading
  /// method determines how resources behave at runtime.
  /// For example, whether calling `acquire` should return
  /// a complete `Resource<T>` but block OR an incomplete
  /// `Resource<T>` but return immediately.
  fn acquire(
    &self,
    provider: &Arc<P>,
    location: &str,
  ) -> Arc<Mutex<Resource<T>>>;
}

pub trait ResourceProvider<T>
{
  /// Acquires the `Resource<T>`. How the `Resource<T>` is acquired
  /// is determined by the implementation. This provides a standard
  /// interface for acquiring resources from directories, archives,
  /// etc. In most circumstances, the implementation should NOT
  /// worry about concurrency. This should return a complete
  /// `Resource<T>` instance when possible.
  fn acquire(&self, location: &str) -> Resource<T>;

  /// Updates the `Resource<T>`. This is usually when `Resource<T>`
  /// cannot be completely loaded during `acquire`. For example,
  /// PCM audio is decoded and converted during `update`.
  fn update(&self, resource: &mut Resource<T>);
}

pub struct ResourceStorage<R, P: ResourceProvider<R>, M: ResourceMethod<R, P>>
{
  resources: HashMap<String, Arc<Mutex<Resource<R>>>>,
  provider:  Arc<P>,
  method:    M,
}

impl<R, M: ResourceMethod<R, P>, P: ResourceProvider<R>>
  ResourceStorage<R, P, M>
{
  pub fn new(provider: P, method: M) -> ResourceStorage<R, P, M>
  {
    ResourceStorage {
      resources: HashMap::new(),
      provider: Arc::new(provider),
      method,
    }
  }

  pub fn acquire(&mut self, location: &str) -> Arc<Mutex<Resource<R>>>
  {
    // `self.provider` and `self.method` are required within `or_insert_with`.
    // However, `self` is mutably borrowed by `or_insert_with`. A splitting
    // borrow allows struct members to be disjointly borrowed. This
    // satisfies the requirments of the borrow checker.
    let (provider, method) = (&self.provider, &self.method);

    #[rustfmt::skip]
    // `or_insert_with` returns a matching element. Otherwise, in the case
    // of no matching elements, construct and return a new element.
    self.resources.entry(String::from(location)).or_insert_with(
      move || method.acquire(provider, location)
    ).clone()
  }

  pub fn update(&mut self)
  {
    // Some `ResourceProvider<R>` implementations provide `update`. This allows
    // `ResourceProvider<R>` to perform work on `self.resources`. The most notable
    // implementation is `StreamMethod<R,P>`. `StreamMethod<R,P>` may stream
    // when the `resource` requests more data.
    for (_, resource) in self.resources.iter_mut() {
      self.provider.update(&mut resource.lock().unwrap());
    }

    // Retain resources within `self.resources` with more than one strong reference.
    // If a resource only has strong one reference, then there exist no external
    // `std::sync::Arc`s that refer to this resource. Therefore, the resource
    // can be released.
    self
      .resources
      .retain(|_, resource| Arc::strong_count(&resource) > 1);
  }
}

pub struct StreamMethod;

impl StreamMethod
{
  pub fn new() -> StreamMethod
  {
    StreamMethod
  }
}

impl<R, P: ResourceProvider<R>> ResourceMethod<R, P> for StreamMethod
{
  fn acquire(
    &self,
    provider: &Arc<P>,
    location: &str,
  ) -> Arc<Mutex<Resource<R>>>
  {
    // `provider.acquire` constructs a `Resource` which might be a valid resource.
    // This is then wrapped within an `Arc<Mutex<R>>` and returned. It is assumed
    // that the caller takes ownership of the `Resource`.
    Arc::new(Mutex::new(provider.acquire(&location)))
  }
}

pub struct AsyncMethod
{
  thread_pool: ThreadPool,
}

impl AsyncMethod
{
  pub fn new(threads: usize) -> AsyncMethod
  {
    #[rustfmt::skip]
    AsyncMethod {
      thread_pool: ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap()
    }
  }
}

impl<R: Send + 'static, P: ResourceProvider<R> + Send + Sync + 'static>
  ResourceMethod<R, P> for AsyncMethod
{
  fn acquire(
    &self,
    provider: &Arc<P>,
    location: &str,
  ) -> Arc<Mutex<Resource<R>>>
  {
    let resource = Arc::new(Mutex::new(Resource::Load));
    {
      // `resource` and `provider` are required within the closure. However, variables
      // cannot be borrowed by closures (only moved). Thus, the `resource` and `provider`
      // containers are cloned, and the clones moved into the closure.
      let (resource, provider) = (resource.clone(), provider.clone());

      // `&str` is not guaranteed to be `'static` and `&str` cannot be safely moved into
      // the closure. Converting `location` to a `String` because `String` can be moved
      // between `threads`.
      let location = location.to_string();

      self.thread_pool.spawn(move || {
        // `*resource.lock` occurs after `provider.acquire`. Therefore, `provider.acquire`
        // will execute without acquiring the `Mutex`. This acquires the `Mutex` for the
        // least possible amount of time.
        *resource.lock().unwrap() = provider.acquire(&location);
      })
    }
    resource
  }
}
