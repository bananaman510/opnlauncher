package net.fabricmc.fabric.api.event;

/**
 * Binary-compatible subset for Fabric API 26.3 event consumers.
 * Only register and invoker are provided by the Horizon lifecycle bridge.
 */
public abstract class Event<T> {
    protected volatile T invoker;

    public final T invoker() {
        return invoker;
    }

    public abstract void register(T listener);
}
