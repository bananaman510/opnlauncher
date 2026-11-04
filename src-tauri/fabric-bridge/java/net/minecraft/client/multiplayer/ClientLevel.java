package net.minecraft.client.multiplayer;

import net.minecraft.world.level.Level;

/** Read-only 26.3 world snapshot supplied by the Horizon game thread. */
public final class ClientLevel extends Level {

    public long getOverworldClockTime() {
        return getDayTime();
    }

    public void updateSnapshot(long dayTime, long gameTime,
                               int minBuildHeight, int maxBuildHeight,
                               boolean raining, boolean thundering,
                               float rainLevelPrevious, float rainLevelCurrent,
                               float thunderLevelPrevious, float thunderLevelCurrent,
                               String nearbyBlocks, double playerX, double playerY, double playerZ) {
        updateEnvironmentSnapshot(dayTime, gameTime, minBuildHeight, maxBuildHeight,
                raining, thundering, rainLevelPrevious, rainLevelCurrent,
                thunderLevelPrevious, thunderLevelCurrent);
        updateNearbyBlockSnapshot(nearbyBlocks, playerX, playerY, playerZ);
    }
}
