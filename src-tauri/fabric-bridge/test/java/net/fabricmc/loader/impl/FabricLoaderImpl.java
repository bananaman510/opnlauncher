package net.fabricmc.loader.impl;

import java.net.URL;
import java.net.URLClassLoader;
import java.nio.file.Path;
import java.util.function.Consumer;
import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.api.ModInitializer;
import org.opnlauncher.horizonfabric.HorizonFabricBridgeTest;

public final class FabricLoaderImpl {
    public static final FabricLoaderImpl INSTANCE = new FabricLoaderImpl();
    private static ClassLoader probeClassLoader;
    private static Class<?> probeType;

    private FabricLoaderImpl() {}

    public <T> void invokeEntrypoints(String key, Class<T> type, Consumer<? super T> invoker) {
        if ("main".equals(key) && type == ModInitializer.class) {
            invoker.accept(type.cast((ModInitializer) HorizonFabricBridgeTest::recordMainEntrypoint));
        } else if ("client".equals(key) && type == ClientModInitializer.class) {
            invoker.accept(type.cast(loadProbeMod()));
        }
    }

    private static Object loadProbeMod() {
        try {
            String jarPath = System.getProperty("opn.fabric.test.probe.jar");
            if (jarPath == null || jarPath.isBlank()) {
                throw new IllegalStateException("the unchanged probe JAR path was not provided");
            }
            URL jar = Path.of(jarPath).toUri().toURL();
            probeClassLoader = new URLClassLoader(new URL[] { jar }, FabricLoaderImpl.class.getClassLoader());
            probeType = Class.forName(
                    "org.opnlauncher.horizonfabricprobe.FabricBridgeProbeMod", true, probeClassLoader);
            return probeType.getConstructor().newInstance();
        } catch (ReflectiveOperationException | java.io.IOException error) {
            throw new IllegalStateException("the unchanged probe JAR could not be loaded", error);
        }
    }

    public static int probeTickCount() {
        try {
            return (int) probeType.getMethod("tickCount").invoke(null);
        } catch (ReflectiveOperationException error) {
            throw new IllegalStateException("the probe tick count could not be read", error);
        }
    }

    public static int probeStartCount() {
        try {
            return (int) probeType.getMethod("startCount").invoke(null);
        } catch (ReflectiveOperationException error) {
            throw new IllegalStateException("the probe start count could not be read", error);
        }
    }

    public static int probeRegisteredKeyCode() {
        try {
            return (int) probeType.getMethod("registeredKeyCode").invoke(null);
        } catch (ReflectiveOperationException error) {
            throw new IllegalStateException("the probe key mapping could not be read", error);
        }
    }
}
