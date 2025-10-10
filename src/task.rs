use std::cell::RefCell;
use std::panic;
use std::rc::Rc;
use std::time::Instant;

use mlua::{
    Either, ExternalError, Function, Lua, MetaMethod, MultiValue, Result, Table, UserData, UserDataFields,
    UserDataMethods, UserDataRef, UserDataRegistry, Value,
};
use tokio::task::{AbortHandle, JoinHandle, JoinSet};
use tokio::time::{Instant as TokioInstant, MissedTickBehavior};
use tokio_util::time::FutureExt as _;

use crate::time::Duration;

#[derive(Clone, Default)]
struct Params {
    name: Option<String>,
    timeout: Option<Duration>,
}

pub struct TaskHandle {
    name: Option<String>,
    started: Rc<RefCell<Option<Instant>>>,
    elapsed: Rc<RefCell<Option<Duration>>>,
    handle: Either<Option<JoinHandle<Result<Value>>>, AbortHandle>,
}

impl UserData for TaskHandle {
    fn register(registry: &mut UserDataRegistry<Self>) {
        registry.add_field_method_get("id", |_, this| match this.handle.as_ref() {
            Either::Left(jh) => Ok(jh.as_ref().map(|h| h.id().to_string())),
            Either::Right(ah) => Ok(Some(ah.id().to_string())),
        });

        registry.add_field_method_get("name", |lua, this| lua.pack(this.name.as_deref()));

        registry.add_async_method_mut("join", |_, mut this, ()| async move {
            if this.handle.is_right() {
                return Ok(Err("cannot join grouped task".into_lua_err()));
            }
            match this.handle.as_mut().left().and_then(|h| h.take()) {
                Some(jh) => match jh.await {
                    Ok(res) => Ok(res),
                    Err(err) if err.is_panic() => panic::resume_unwind(err.into_panic()),
                    Err(err) => Ok(Err(err.into_lua_err())),
                },
                None => Ok(Err("task already joined".into_lua_err())),
            }
        });

        registry.add_async_method("abort", |_, this, ()| async move {
            match this.handle.as_ref() {
                Either::Left(Some(jh)) => {
                    jh.abort();
                }
                Either::Left(None) => {}
                Either::Right(ah) => {
                    ah.abort();
                }
            }
            Ok(())
        });

        registry.add_method("elapsed", |_, this, ()| match *this.elapsed.borrow() {
            Some(dur) => Ok(Some(dur)),
            None => Ok(this.started.borrow().map(|s| Duration(s.elapsed()))),
        });

        registry.add_method("is_finished", |_, this, ()| match this.handle.as_ref() {
            Either::Left(Some(jh)) => Ok(jh.is_finished()),
            Either::Left(None) => Ok(true),
            Either::Right(ah) => Ok(ah.is_finished()),
        });
    }
}

pub struct Task {
    func: Function,
    params: Params,
}

impl Task {
    fn new(func: Function, params: Option<Table>) -> Result<Self> {
        let name: Option<String> = opt_param!(params, "name")?;
        let timeout: Option<Duration> = opt_param!(params, "timeout")?;
        Ok(Self {
            func,
            params: Params { name, timeout },
        })
    }
}

impl UserData for Task {}

pub struct Group(JoinSet<Result<Value>>);

impl Group {
    fn new() -> Self {
        Group(JoinSet::new())
    }
}

impl UserData for Group {
    fn register(registry: &mut UserDataRegistry<Self>) {
        registry.add_method_mut(
            "spawn",
            |_, this, (func, args): (Either<Function, UserDataRef<Task>>, MultiValue)| {
                let Params { name, timeout } = (func.as_ref())
                    .right()
                    .map_or(Params::default(), |ud| ud.params.clone());

                let started = Rc::new(RefCell::new(None));
                let elapsed = Rc::new(RefCell::new(None));
                let (started2, elapsed2) = (started.clone(), elapsed.clone());

                let fut = match func {
                    Either::Left(f) => f.call_async(args),
                    Either::Right(ud) => ud.func.call_async(args),
                };

                let abort_handle = this.0.spawn_local(async move {
                    *started2.borrow_mut() = Some(Instant::now());
                    defer! {
                        *elapsed2.borrow_mut() = Some(Duration(started2.borrow().unwrap().elapsed()));
                    }

                    let result = match timeout {
                        Some(dur) => fut.timeout(dur.0).await,
                        None => Ok(fut.await),
                    };
                    result
                        .map_err(|_| "task exceeded timeout".into_lua_err())
                        .flatten()
                });

                Ok(TaskHandle {
                    name,
                    started,
                    elapsed,
                    handle: Either::Right(abort_handle),
                })
            },
        );

        registry.add_method("len", |_, this, ()| Ok(this.0.len()));

        registry.add_async_method_mut("join_next", |_, mut this, ()| async move {
            match this.0.join_next().await {
                Some(Ok(res)) => Ok(Ok(Some(lua_try!(res)))),
                Some(Err(err)) if err.is_panic() => panic::resume_unwind(err.into_panic()),
                Some(Err(err)) => Ok(Err(err.to_string())),
                None => Ok(Ok(None)),
            }
        });

        registry.add_async_method_mut("join_all", |_, mut this, ()| async move {
            let mut results = Vec::with_capacity(this.0.len());
            while let Some(res) = this.0.join_next().await {
                match res {
                    Ok(Ok(val)) => results.push(val),
                    Ok(Err(err)) => results.push(Value::Error(Box::new(err))),
                    Err(err) if err.is_panic() => panic::resume_unwind(err.into_panic()),
                    Err(err) => results.push(Value::Error(Box::new(err.into_lua_err()))),
                }
            }
            Ok(results)
        });

        registry.add_method_mut("abort_all", |_, this, ()| {
            this.0.abort_all();
            Ok(())
        });

        registry.add_method_mut("detach_all", |_, this, ()| {
            this.0.detach_all();
            Ok(())
        });

        registry.add_meta_method(MetaMethod::Len, |_, this, ()| Ok(this.0.len()));
    }
}

fn spawn_inner(params: Params, fut: impl Future<Output = Result<Value>> + 'static) -> Result<TaskHandle> {
    let Params { name, timeout } = params;

    let started = Rc::new(RefCell::new(None));
    let elapsed = Rc::new(RefCell::new(None));
    let (started2, elapsed2) = (started.clone(), elapsed.clone());

    let handle = tokio::task::spawn_local(async move {
        *started2.borrow_mut() = Some(Instant::now());
        defer! {
            *elapsed2.borrow_mut() = Some(Duration(started2.borrow().unwrap().elapsed()));
        }

        let result = match timeout {
            Some(dur) => fut.timeout(dur.0).await,
            None => Ok(fut.await),
        };
        result
            .map_err(|_| "task exceeded timeout".into_lua_err())
            .flatten()
    });

    Ok(TaskHandle {
        name,
        started,
        elapsed,
        handle: Either::Left(Some(handle)),
    })
}

pub fn spawn(_: &Lua, (func, args): (Either<Function, UserDataRef<Task>>, MultiValue)) -> Result<TaskHandle> {
    let params = (func.as_ref())
        .right()
        .map_or(Params::default(), |ud| ud.params.clone());

    spawn_inner(params, async move {
        match func {
            Either::Left(f) => f.call_async(args).await,
            Either::Right(ud) => ud.func.call_async(args).await,
        }
    })
}

pub fn spawn_every(
    _: &Lua,
    (dur, func, args): (Duration, Either<Function, UserDataRef<Task>>, MultiValue),
) -> Result<TaskHandle> {
    let (func, params) = match func {
        Either::Left(f) => (f, Params::default()),
        Either::Right(ud) => (ud.func.clone(), ud.params.clone()),
    };

    spawn_inner(params, async move {
        let mut interval = tokio::time::interval_at(TokioInstant::now() + dur.0, dur.0);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            func.call_async::<()>(args.clone()).await?;
        }
    })
}

pub async fn sleep(_: Lua, dur: Duration) -> Result<()> {
    tokio::time::sleep(dur.0).await;
    Ok(())
}

pub async fn yield_now(_: Lua, _: ()) -> Result<()> {
    tokio::task::yield_now().await;
    Ok(())
}

/// A loader for the `task` module.
fn loader(lua: &Lua) -> Result<Table> {
    let t = lua.create_table()?;
    t.set("create", Function::wrap(Task::new))?;
    t.set("group", Function::wrap_raw(Group::new))?;
    t.set("spawn", lua.create_function(spawn)?)?;
    t.set("spawn_every", lua.create_function(spawn_every)?)?;
    t.set("sleep", lua.create_async_function(sleep)?)?;
    t.set("yield", lua.create_async_function(yield_now)?)?;
    Ok(t)
}

/// Registers the `task` module in the given Lua state.
pub fn register(lua: &Lua, name: Option<&str>) -> Result<Table> {
    let name = name.unwrap_or("@task");
    let value = loader(lua)?;
    lua.register_module(name, &value)?;
    Ok(value)
}
