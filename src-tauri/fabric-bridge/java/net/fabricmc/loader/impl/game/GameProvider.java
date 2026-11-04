package net.fabricmc.loader.impl.game;

import java.nio.file.Path;
import java.util.Collection;
import java.util.List;
import java.util.Objects;
import java.util.Set;
import net.fabricmc.loader.api.metadata.ModMetadata;
import net.fabricmc.loader.impl.game.patch.GameTransformer;
import net.fabricmc.loader.impl.launch.FabricLauncher;
import net.fabricmc.loader.impl.util.Arguments;

/** Compile-only Fabric Loader SPI declaration; the application does not bundle this class. */
public interface GameProvider {
    String getGameId();
    String getGameName();
    String getRawGameVersion();
    String getNormalizedGameVersion();
    Collection<BuiltinMod> getBuiltinMods();
    String getEntrypoint();
    Path getLaunchDirectory();
    boolean requiresUrlClassLoader();
    Set<BuiltinTransform> getBuiltinTransforms(String className);
    boolean isEnabled();
    boolean locateGame(FabricLauncher launcher, String[] args);
    void initialize(FabricLauncher launcher);
    GameTransformer getEntrypointTransformer();
    void unlockClassPath(FabricLauncher launcher);
    void launch(ClassLoader loader);
    Arguments getArguments();
    String[] getLaunchArguments(boolean sanitize);

    enum BuiltinTransform { STRIP_ENVIRONMENT, WIDEN_ALL_PACKAGE_ACCESS, CLASS_TWEAKS }

    final class BuiltinMod {
        public BuiltinMod(List<Path> paths, ModMetadata metadata) {
            Objects.requireNonNull(paths);
            Objects.requireNonNull(metadata);
        }
    }
}
