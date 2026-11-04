package org.opnlauncher.horizonfabric;

import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Collection;
import java.util.List;
import java.util.Map;
import java.util.Set;
import net.fabricmc.loader.api.metadata.ModMetadata;
import net.fabricmc.loader.impl.game.GameProvider;
import net.fabricmc.loader.impl.game.patch.GameTransformer;
import net.fabricmc.loader.impl.launch.FabricLauncher;
import net.fabricmc.loader.impl.util.Arguments;

/**
 * Fabric Loader provider for the experimental Rust Horizon event bridge.
 * It prepares Fabric mods but never starts Minecraft's Java main class.
 */
public final class HorizonFabricGameProvider implements GameProvider {
    private static final String GAME_VERSION = "26.3";
    private static final String FABRIC_API_VERSION = "0.165.2+26.3";
    private final GameTransformer transformer = new GameTransformer();

    @Override public String getGameId() { return "minecraft"; }
    @Override public String getGameName() { return "Horizon Fabric compatibility bridge"; }
    @Override public String getRawGameVersion() { return GAME_VERSION; }
    @Override public String getNormalizedGameVersion() { return GAME_VERSION; }
    @Override public String getEntrypoint() { return HorizonFabricBridgeMain.class.getName(); }
    @Override public Path getLaunchDirectory() {
        return Paths.get(System.getProperty("opn.fabric.gameDir", ".")).toAbsolutePath();
    }
    @Override public boolean requiresUrlClassLoader() { return false; }
    @Override public Set<BuiltinTransform> getBuiltinTransforms(String className) { return Set.of(); }
    @Override public boolean isEnabled() { return Boolean.getBoolean("fabric.skipMcProvider"); }
    @Override public boolean locateGame(FabricLauncher launcher, String[] args) { return isEnabled(); }
    @Override public void initialize(FabricLauncher launcher) {
        transformer.locateEntrypoints(launcher, List.of());
    }
    @Override public GameTransformer getEntrypointTransformer() { return transformer; }
    @Override public void unlockClassPath(FabricLauncher launcher) {}
    @Override public void launch(ClassLoader loader) {
        HorizonFabricBridgeMain.run(loader);
    }
    @Override public Arguments getArguments() { return new Arguments(); }
    @Override public String[] getLaunchArguments(boolean sanitize) { return new String[0]; }

    @Override
    public Collection<BuiltinMod> getBuiltinMods() {
        return List.of(
                new BuiltinMod(List.of(), builtinMetadata("minecraft", "Minecraft facade", GAME_VERSION,
                        "Narrow client and HUD method facades mapped to the native Horizon game.")),
                new BuiltinMod(List.of(), builtinMetadata("fabric-api", "Fabric API lifecycle shim", FABRIC_API_VERSION,
                        "OpnLauncher maps client tick events and selected read-only client snapshots.")),
                new BuiltinMod(List.of(), builtinMetadata("fabric-api-base", "Fabric API base shim", FABRIC_API_VERSION,
                        "Provides the event registration subset used by the Horizon bridge.")),
                new BuiltinMod(List.of(), builtinMetadata("fabric-lifecycle-events-v1", "Fabric lifecycle events shim", FABRIC_API_VERSION,
                        "Only ClientTickEvents.START_CLIENT_TICK and END_CLIENT_TICK are mapped.")),
                new BuiltinMod(List.of(), builtinMetadata("fabric-key-mapping-api-v1", "Fabric key mapping shim", FABRIC_API_VERSION,
                        "Registered keyboard mappings and namespaced categories map to native Horizon press and held-key snapshots; controls-menu rebinding and mouse/controller input are not mapped.")));
    }

    private static ModMetadata builtinMetadata(String id, String name, String version, String description) {
        try {
            ClassLoader loader = HorizonFabricGameProvider.class.getClassLoader();
            Class<?> metadataType = Class.forName("net.fabricmc.loader.api.metadata.ModMetadata", true, loader);
            InvocationHandler handler = (proxy, method, args) ->
                    metadataValue(loader, method, proxy, args, id, name, version, description);
            return (ModMetadata) Proxy.newProxyInstance(loader, new Class<?>[] { metadataType }, handler);
        } catch (ReflectiveOperationException error) {
            throw new IllegalStateException("Fabric Loader's metadata API is unavailable", error);
        }
    }

    private static Object metadataValue(
            ClassLoader loader, Method method, Object proxy, Object[] args,
            String id, String name, String version, String description)
            throws ReflectiveOperationException {
        return switch (method.getName()) {
            case "getId" -> id;
            case "getType" -> "builtin";
            case "getName" -> name;
            case "getDescription" -> description;
            case "getVersion" -> Class.forName("net.fabricmc.loader.api.Version", true, loader)
                    .getMethod("parse", String.class).invoke(null, version);
            case "getEnvironment" -> enumConstant(
                    Class.forName("net.fabricmc.loader.api.metadata.ModEnvironment", true, loader), "UNIVERSAL");
            case "getProvides", "getAuthors", "getContributors", "getLicense", "getJars",
                    "getEntrypoints", "getLanguageAdapterDefinitions", "getCustomValues",
                    "getDependencies", "getDepends", "getRecommends", "getSuggests", "getConflicts", "getBreaks",
                    "getMixinConfigs", "getAccessWideners" -> emptyValue(method.getReturnType());
            case "getContact" -> emptyInterfaceValue(loader, method.getReturnType());
            case "getIconPath" -> java.util.Optional.empty();
            case "getCustomValue" -> null;
            case "containsCustomValue", "containsCustomElement" -> false;
            case "toString" -> "OpnLauncher built-in " + id + " metadata";
            case "hashCode" -> System.identityHashCode(proxy);
            case "equals" -> args != null && args.length == 1 && proxy == args[0];
            default -> defaultValue(method.getReturnType());
        };
    }

    private static Object enumConstant(Class<?> type, String name) {
        Object[] values = type.getEnumConstants();
        if (values != null) {
            for (Object value : values) {
                if (((Enum<?>) value).name().equals(name)) return value;
            }
        }
        return null;
    }

    private static Object emptyValue(Class<?> type) {
        if (Map.class.isAssignableFrom(type)) return Map.of();
        if (Set.class.isAssignableFrom(type)) return Set.of();
        if (Collection.class.isAssignableFrom(type)) return List.of();
        if (type == java.util.Optional.class) return java.util.Optional.empty();
        return defaultValue(type);
    }

    private static Object emptyInterfaceValue(ClassLoader loader, Class<?> type) {
        if (!type.isInterface()) return null;
        InvocationHandler empty = (proxy, method, args) -> switch (method.getName()) {
            case "toString" -> "Empty Fabric metadata contact";
            case "hashCode" -> System.identityHashCode(proxy);
            case "equals" -> args != null && args.length == 1 && proxy == args[0];
            default -> emptyValue(method.getReturnType());
        };
        return Proxy.newProxyInstance(loader, new Class<?>[] { type }, empty);
    }

    private static Object defaultValue(Class<?> type) {
        if (!type.isPrimitive()) return null;
        if (type == boolean.class) return false;
        if (type == char.class) return '\0';
        if (type == byte.class) return (byte) 0;
        if (type == short.class) return (short) 0;
        if (type == int.class) return 0;
        if (type == long.class) return 0L;
        if (type == float.class) return 0f;
        if (type == double.class) return 0d;
        return null;
    }
}
