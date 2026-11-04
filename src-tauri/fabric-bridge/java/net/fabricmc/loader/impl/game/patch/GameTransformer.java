package net.fabricmc.loader.impl.game.patch;

import java.nio.file.Path;
import java.util.List;
import net.fabricmc.loader.impl.launch.FabricLauncher;

/** Compile-only Fabric Loader SPI declaration; the application does not bundle this class. */
public class GameTransformer {
    public GameTransformer(GamePatch... patches) {}

    public void locateEntrypoints(FabricLauncher launcher, List<Path> gameJars) {}
}
