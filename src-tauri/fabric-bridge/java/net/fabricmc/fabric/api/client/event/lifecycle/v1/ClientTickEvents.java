package net.fabricmc.fabric.api.client.event.lifecycle.v1;

import java.util.concurrent.CopyOnWriteArrayList;
import net.fabricmc.fabric.api.event.Event;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;

/**
 * Narrow Fabric API 26.3 mapping: client start/end ticks are backed by
 * Horizon Rust client hooks. Level lifecycle events reject listeners clearly.
 */
public final class ClientTickEvents {
    private ClientTickEvents() {}

    public static final Event<StartTick> START_CLIENT_TICK = new StartClientTickEvent();
    public static final Event<EndTick> END_CLIENT_TICK = new EndClientTickEvent();
    public static final Event<StartLevelTick> START_LEVEL_TICK = unsupported("START_LEVEL_TICK");
    public static final Event<EndLevelTick> END_LEVEL_TICK = unsupported("END_LEVEL_TICK");

    private static <T> Event<T> unsupported(String eventName) {
        return new Event<>() {
            @Override
            public void register(T listener) {
                throw new UnsupportedOperationException(
                        "OpnLauncher Fabric bridge does not map ClientTickEvents." + eventName);
            }
        };
    }

    private static final class EndClientTickEvent extends Event<EndTick> {
        private final CopyOnWriteArrayList<EndTick> listeners = new CopyOnWriteArrayList<>();

        private EndClientTickEvent() {
            invoker = client -> {
                for (EndTick listener : listeners) {
                    listener.onEndTick(client);
                }
            };
        }

        @Override
        public void register(EndTick listener) {
            if (listener == null) throw new IllegalArgumentException("Fabric tick listener cannot be null");
            listeners.add(listener);
        }
    }

    private static final class StartClientTickEvent extends Event<StartTick> {
        private final CopyOnWriteArrayList<StartTick> listeners = new CopyOnWriteArrayList<>();

        private StartClientTickEvent() {
            invoker = client -> {
                for (StartTick listener : listeners) {
                    listener.onStartTick(client);
                }
            };
        }

        @Override
        public void register(StartTick listener) {
            if (listener == null) throw new IllegalArgumentException("Fabric tick listener cannot be null");
            listeners.add(listener);
        }
    }

    @FunctionalInterface
    public interface StartTick {
        void onStartTick(Minecraft client);
    }

    @FunctionalInterface
    public interface EndTick {
        void onEndTick(Minecraft client);
    }

    @FunctionalInterface
    public interface StartLevelTick {
        void onStartTick(ClientLevel level);
    }

    @FunctionalInterface
    public interface EndLevelTick {
        void onEndTick(ClientLevel level);
    }
}
