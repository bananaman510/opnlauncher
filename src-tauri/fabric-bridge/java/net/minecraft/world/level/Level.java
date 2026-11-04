package net.minecraft.world.level;

import java.util.HashMap;
import java.util.Map;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.block.state.BlockState;

/** Read-only environmental snapshot from the native Horizon world. */
public abstract class Level {
    private long dayTime;
    private long gameTime;
    private int minBuildHeight;
    private int maxBuildHeight;
    private boolean raining;
    private boolean thundering;
    private float rainLevelPrevious;
    private float rainLevelCurrent;
    private float thunderLevelPrevious;
    private float thunderLevelCurrent;
    private Map<String, BlockState> nearbyBlocks = Map.of();

    public long getDayTime() { return dayTime; }
    public long getGameTime() { return gameTime; }
    public int getMinBuildHeight() { return minBuildHeight; }
    public int getMaxBuildHeight() { return maxBuildHeight; }
    public boolean isRaining() { return raining; }
    public boolean isThundering() { return thundering; }

    /** Returns a block only when it is present in the current bounded Horizon snapshot. */
    public BlockState getBlockState(BlockPos pos) {
        if (pos == null) throw new IllegalArgumentException("Block position is required");
        BlockState state = nearbyBlocks.get(positionKey(pos.getX(), pos.getY(), pos.getZ()));
        if (state == null) {
            throw new UnsupportedOperationException("Block queries are mapped only within one block of the local player");
        }
        return state;
    }

    public float getRainLevel(float partialTick) {
        float partial = clampPartialTick(partialTick);
        return rainLevelPrevious + (rainLevelCurrent - rainLevelPrevious) * partial;
    }

    public float getThunderLevel(float partialTick) {
        float partial = clampPartialTick(partialTick);
        return thunderLevelPrevious + (thunderLevelCurrent - thunderLevelPrevious) * partial;
    }

    private static float clampPartialTick(float partialTick) {
        if (!Float.isFinite(partialTick)) throw new IllegalArgumentException("Partial tick must be finite");
        return Math.max(0.0f, Math.min(1.0f, partialTick));
    }

    protected final void updateEnvironmentSnapshot(long dayTime, long gameTime,
                                                   int minBuildHeight, int maxBuildHeight,
                                                   boolean raining, boolean thundering,
                                                   float rainLevelPrevious, float rainLevelCurrent,
                                                   float thunderLevelPrevious, float thunderLevelCurrent) {
        this.dayTime = dayTime;
        this.gameTime = gameTime;
        this.minBuildHeight = minBuildHeight;
        this.maxBuildHeight = maxBuildHeight;
        this.raining = raining;
        this.thundering = thundering;
        this.rainLevelPrevious = rainLevelPrevious;
        this.rainLevelCurrent = rainLevelCurrent;
        this.thunderLevelPrevious = thunderLevelPrevious;
        this.thunderLevelCurrent = thunderLevelCurrent;
    }

    protected final void updateNearbyBlockSnapshot(String encoded, double playerX, double playerY, double playerZ) {
        if ("-".equals(encoded)) {
            nearbyBlocks = Map.of();
            return;
        }
        if (encoded == null || encoded.length() > 8192
                || !Double.isFinite(playerX) || !Double.isFinite(playerY) || !Double.isFinite(playerZ)) {
            throw new IllegalArgumentException("Block snapshot is malformed or too large");
        }
        int centerX = floorCoordinate(playerX);
        int centerY = floorCoordinate(playerY);
        int centerZ = floorCoordinate(playerZ);
        String[] samples = encoded.split(";", -1);
        if (samples.length > 27) throw new IllegalArgumentException("Block snapshot exceeds its 3x3x3 limit");
        Map<String, BlockState> parsed = new HashMap<>(samples.length);
        for (String sample : samples) {
            String[] fields = sample.split(",", -1);
            if (fields.length != 7) throw new IllegalArgumentException("Block snapshot entry has an invalid field count");
            int x = parseCoordinate(fields[0]);
            int y = parseCoordinate(fields[1]);
            int z = parseCoordinate(fields[2]);
            if (Math.abs((long) x - centerX) > 1 || Math.abs((long) y - centerY) > 1
                    || Math.abs((long) z - centerZ) > 1) {
                throw new IllegalArgumentException("Block snapshot entry is outside the local 3x3x3 region");
            }
            if (!fields[3].matches("[a-z0-9_.-]+:[a-z0-9_./-]+")) {
                throw new IllegalArgumentException("Block snapshot contains an invalid block identifier");
            }
            BlockState state = new BlockState(fields[3], parseFlag(fields[4]), parseFlag(fields[5]), parseFlag(fields[6]));
            if (parsed.putIfAbsent(positionKey(x, y, z), state) != null) {
                throw new IllegalArgumentException("Block snapshot contains a duplicate position");
            }
        }
        nearbyBlocks = Map.copyOf(parsed);
    }

    private static int floorCoordinate(double value) {
        if (value < Integer.MIN_VALUE || value > Integer.MAX_VALUE) {
            throw new IllegalArgumentException("Player position exceeds block snapshot coordinate bounds");
        }
        return (int) Math.floor(value);
    }

    private static int parseCoordinate(String value) {
        try { return Integer.parseInt(value); }
        catch (NumberFormatException error) { throw new IllegalArgumentException("Block snapshot coordinate is invalid", error); }
    }

    private static boolean parseFlag(String value) {
        if ("0".equals(value)) return false;
        if ("1".equals(value)) return true;
        throw new IllegalArgumentException("Block snapshot flag is invalid");
    }

    private static String positionKey(int x, int y, int z) { return x + "," + y + "," + z; }
}
