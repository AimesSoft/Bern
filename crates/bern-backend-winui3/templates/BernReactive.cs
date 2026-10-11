// Bern runtime for WinUI 3: reactive cells, effects and scopes on the UI
// thread. Generated build input; do not edit.
//
// The same model as the other runtimes: an effect runs now and again
// whenever a cell it read changes; scopes own effects, cleanups and child
// scopes, and disposing a scope ends them all. Effects created while an
// effect runs belong to that run and end before the next one.

using System;
using System.Collections.Generic;

namespace Bern;

/// Something an effect read.
interface ISource
{
    void Forget(Effect effect);
}

public sealed class Cell<T> : ISource
{
    T value;
    readonly List<WeakReference<Effect>> readers = new();

    public Cell(T value) => this.value = value;

    /// The value, recording a dependency of the running effect.
    public T Get()
    {
        Track();
        return value;
    }

    /// Read without recording a dependency.
    public T Peek() => value;

    void Track()
    {
        var effect = Reactive.Observer;
        if (effect is null) return;
        foreach (var reader in readers)
            if (reader.TryGetTarget(out var existing) && ReferenceEquals(existing, effect)) return;
        readers.Add(new WeakReference<Effect>(effect));
        effect.Sources.Add(this);
    }

    /// Change the value; readers re-run when it differs.
    public void Set(T next)
    {
        if (EqualityComparer<T>.Default.Equals(value, next)) return;
        value = next;
        Notify();
    }

    /// Re-run readers after changing the value in place.
    public void Changed() => Notify();

    void Notify()
    {
        var current = new List<Effect>();
        foreach (var reader in readers)
            if (reader.TryGetTarget(out var effect)) current.Add(effect);
        readers.Clear();
        foreach (var effect in current) effect.Schedule();
    }

    void ISource.Forget(Effect effect) =>
        readers.RemoveAll(reader => !reader.TryGetTarget(out var target) || ReferenceEquals(target, effect));
}

/// Owns effects, cleanups and child scopes.
public sealed class Scope
{
    readonly List<Action> cleanups = new();
    public bool IsDisposed { get; private set; }

    /// A scope owned by the current one: it ends when the current one does.
    public static Scope Owned()
    {
        var scope = new Scope();
        Reactive.CurrentScope?.OnDispose(scope.Dispose);
        return scope;
    }

    public void OnDispose(Action cleanup)
    {
        if (IsDisposed) cleanup();
        else cleanups.Add(cleanup);
    }

    public void Dispose()
    {
        if (IsDisposed) return;
        IsDisposed = true;
        var pending = cleanups.ToArray();
        cleanups.Clear();
        for (var i = pending.Length - 1; i >= 0; i--)
        {
            try { pending[i](); }
            catch (Exception error) { Reactive.Report(error); }
        }
    }

    /// Run `body` with this scope as the current one.
    public void Run(Action body)
    {
        var saved = Reactive.CurrentScope;
        Reactive.CurrentScope = this;
        try { body(); }
        finally { Reactive.CurrentScope = saved; }
    }
}

public sealed class Effect
{
    readonly Action body;
    internal readonly List<ISource> Sources = new();
    Scope? inner;
    bool disposed;
    bool queued;

    internal Effect(Action body) => this.body = body;

    internal void Schedule()
    {
        if (disposed) return;
        if (Reactive.Depth > 0)
        {
            if (!queued)
            {
                queued = true;
                Reactive.Queue.Add(this);
            }
        }
        else Run();
    }

    internal void Run()
    {
        queued = false;
        if (disposed) return;
        ClearSources();
        // Scopes created by the previous run end before the next one.
        inner?.Dispose();
        inner = new Scope();
        var saved = Reactive.Observer;
        Reactive.Observer = this;
        try { inner.Run(body); }
        catch (Exception error) { Reactive.Report(error); }
        finally { Reactive.Observer = saved; }
    }

    void ClearSources()
    {
        foreach (var source in Sources) source.Forget(this);
        Sources.Clear();
    }

    internal void Dispose()
    {
        disposed = true;
        ClearSources();
        inner?.Dispose();
        inner = null;
    }
}

public static class Reactive
{
    internal static Effect? Observer;
    internal static Scope? CurrentScope;
    internal static int Depth;
    internal static readonly List<Effect> Queue = new();

    /// Run `body` now and again whenever a cell it read changes, until the
    /// current scope ends.
    public static void Effect(Action body)
    {
        var effect = new Effect(body);
        CurrentScope?.OnDispose(effect.Dispose);
        effect.Run();
    }

    /// Register a cleanup with the current scope.
    public static void OnCleanup(Action cleanup) => CurrentScope?.OnDispose(cleanup);

    /// Apply several changes and re-run each affected effect once afterwards.
    public static void Batch(Action body)
    {
        Depth++;
        try { body(); }
        finally
        {
            Depth--;
            if (Depth == 0)
            {
                while (Queue.Count > 0)
                {
                    var next = Queue.ToArray();
                    Queue.Clear();
                    foreach (var effect in next) effect.Run();
                }
            }
        }
    }

    /// Read cells without recording dependencies.
    public static T Untrack<T>(Func<T> body)
    {
        var saved = Observer;
        Observer = null;
        try { return body(); }
        finally { Observer = saved; }
    }

    public static void Untrack(Action body)
    {
        var saved = Observer;
        Observer = null;
        try { body(); }
        finally { Observer = saved; }
    }

    internal static void Report(Exception error) =>
        Console.Error.WriteLine($"bern: {error}");
}

/// A counter cell for collections whose items cannot be compared.
public sealed class Version
{
    readonly Cell<long> cell = new(0);
    public void Track() => cell.Get();
    public void Bump() => cell.Set(cell.Peek() + 1);
}
