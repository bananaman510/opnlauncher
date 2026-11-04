package org.opnlauncher.horizonfabric;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.PrintWriter;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.net.Socket;
import java.nio.ByteBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashSet;
import java.util.HexFormat;
import java.util.List;
import java.util.Set;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.function.Consumer;

/** Runs Fabric entrypoints and maps a small client API subset to Rust Horizon. */
public final class HorizonFabricBridgeMain {
    private static final double MAX_PLAYER_SPEED = 4.0;
    private static final float MAX_PLAYER_YAW = 360_000.0f;
    private static final float MIN_PLAYER_PITCH = -90.0f;
    private static final float MAX_PLAYER_PITCH = 90.0f;
    private static final Object ACTION_LOCK = new Object();
    private static String pendingOverlayMessage;
    private static boolean pendingOverlayAnimated;
    private static String pendingSystemMessage;
    private static Boolean pendingSprinting;
    private static double[] pendingPlayerVelocity;
    private static float[] pendingPlayerRotation;

    private HorizonFabricBridgeMain() {}

    /** Called only by the mapped Minecraft 26.3 HUD facade. */
    public static void queueOverlayMessage(String message, boolean animate) {
        String clean = cleanMessage(message);
        if (clean == null) return;
        synchronized (ACTION_LOCK) {
            pendingOverlayMessage = clean;
            pendingOverlayAnimated = animate;
        }
    }

    /** Queues plain system chat text for insertion by Horizon's Rust game thread. */
    public static void queueSystemMessage(String message) {
        String clean = cleanMessage(message);
        if (clean == null) return;
        synchronized (ACTION_LOCK) {
            pendingSystemMessage = clean;
        }
    }

    private static String cleanMessage(String message) {
        if (message == null) return null;
        StringBuilder clean = new StringBuilder();
        message.codePoints()
                .filter(codePoint -> !Character.isISOControl(codePoint))
                .limit(256)
                .forEach(clean::appendCodePoint);
        return clean.toString();
    }

    /** Queues one local-player sprint intent for application on Horizon's game thread. */
    public static void queueSprinting(boolean sprinting) {
        synchronized (ACTION_LOCK) {
            pendingSprinting = sprinting;
        }
    }

    /** Queues only finite, bounded player motion; Horizon applies it on its own game thread. */
    public static boolean queuePlayerVelocity(double x, double y, double z) {
        double speedSquared = x * x + y * y + z * z;
        if (!Double.isFinite(x) || !Double.isFinite(y) || !Double.isFinite(z)
                || !Double.isFinite(speedSquared) || speedSquared > MAX_PLAYER_SPEED * MAX_PLAYER_SPEED) {
            return false;
        }
        synchronized (ACTION_LOCK) {
            pendingPlayerVelocity = new double[] {x, y, z};
        }
        return true;
    }

    /** Queues a bounded local look direction for application on Horizon's Rust game thread. */
    public static boolean queuePlayerRotation(float yaw, float pitch) {
        if (!Float.isFinite(yaw) || Math.abs(yaw) > MAX_PLAYER_YAW
                || !Float.isFinite(pitch) || pitch < MIN_PLAYER_PITCH || pitch > MAX_PLAYER_PITCH) {
            return false;
        }
        synchronized (ACTION_LOCK) {
            pendingPlayerRotation = new float[] {yaw, pitch};
        }
        return true;
    }

    private static String takeOverlayAction() {
        synchronized (ACTION_LOCK) {
            if (pendingOverlayMessage == null) return "-";
            byte[] bytes = pendingOverlayMessage.getBytes(StandardCharsets.UTF_8);
            StringBuilder encoded = new StringBuilder(3 + bytes.length * 2);
            encoded.append(pendingOverlayAnimated ? "A1:" : "A0:");
            for (byte value : bytes) encoded.append(String.format("%02x", value & 0xff));
            pendingOverlayMessage = null;
            pendingOverlayAnimated = false;
            return encoded.toString();
        }
    }

    private static String takeSystemMessageAction() {
        synchronized (ACTION_LOCK) {
            if (pendingSystemMessage == null) return "-";
            String encoded = "M:" + HexFormat.of().formatHex(pendingSystemMessage.getBytes(StandardCharsets.UTF_8));
            pendingSystemMessage = null;
            return encoded;
        }
    }

    private static String takeSprintingAction() {
        synchronized (ACTION_LOCK) {
            if (pendingSprinting == null) return "-";
            String encoded = pendingSprinting ? "S1" : "S0";
            pendingSprinting = null;
            return encoded;
        }
    }

    private static String takePlayerVelocityAction() {
        synchronized (ACTION_LOCK) {
            if (pendingPlayerVelocity == null) return "-";
            String encoded = "V:" + pendingPlayerVelocity[0] + ","
                    + pendingPlayerVelocity[1] + "," + pendingPlayerVelocity[2];
            pendingPlayerVelocity = null;
            return encoded;
        }
    }

    private static String takePlayerRotationAction() {
        synchronized (ACTION_LOCK) {
            if (pendingPlayerRotation == null) return "-";
            String encoded = "R:" + pendingPlayerRotation[0] + "," + pendingPlayerRotation[1];
            pendingPlayerRotation = null;
            return encoded;
        }
    }

    public static void run(ClassLoader loader) {
        Thread.currentThread().setContextClassLoader(loader);
        int port;
        try {
            port = Integer.parseInt(System.getProperty("opn.fabric.bridge.port", ""));
        } catch (NumberFormatException error) {
            System.err.println("Horizon Fabric bridge: no valid local bridge port was provided.");
            return;
        }

        if (!invokeEntrypoints(loader, "main", "net.fabricmc.api.ModInitializer", "onInitialize")) return;
        if (!invokeEntrypoints(loader, "client", "net.fabricmc.api.ClientModInitializer", "onInitializeClient")) return;

        try (Socket socket = new Socket("127.0.0.1", port);
             BufferedReader input = new BufferedReader(new InputStreamReader(socket.getInputStream()));
             PrintWriter output = new PrintWriter(socket.getOutputStream(), true)) {
            output.println("ready");
            dispatchTicks(loader, input, output);
        } catch (Exception error) {
            System.err.println("Horizon Fabric bridge: local event connection ended: " + error.getMessage());
        }
    }

    static boolean invokeEntrypoints(ClassLoader loader, String key, String interfaceName, String callbackName) {
        try {
            Class<?> implementationType = Class.forName("net.fabricmc.loader.impl.FabricLoaderImpl", true, loader);
            Object fabricLoader = implementationType.getField("INSTANCE").get(null);
            Class<?> entrypointType = Class.forName(interfaceName, true, loader);
            Method callback = entrypointType.getMethod(callbackName);
            Method invoke = implementationType.getMethod("invokeEntrypoints", String.class, Class.class, Consumer.class);
            Consumer<Object> consumer = entrypoint -> {
                try {
                    callback.invoke(entrypoint);
                } catch (InvocationTargetException error) {
                    throw new IllegalStateException(error.getCause());
                } catch (ReflectiveOperationException error) {
                    throw new IllegalStateException(error);
                }
            };
            invoke.invoke(fabricLoader, key, entrypointType, consumer);
            return true;
        } catch (ClassNotFoundException missing) {
            if ("net.fabricmc.api.ClientModInitializer".equals(interfaceName)) return true;
            System.err.println("Horizon Fabric bridge: Fabric Loader API is incomplete: " + missing.getMessage());
            return false;
        } catch (InvocationTargetException error) {
            System.err.println("Horizon Fabric bridge: one or more '" + key + "' entrypoints failed: " + error.getCause());
            return false;
        } catch (ReflectiveOperationException error) {
            System.err.println("Horizon Fabric bridge: could not run '" + key + "' entrypoints: " + error);
            return false;
        }
    }

    static void dispatchTicks(ClassLoader loader, BufferedReader input, PrintWriter output) {
        Object startInvoker = null;
        Method startCallback = null;
        Object endInvoker = null;
        Method endCallback = null;
        Object client = null;
        AtomicBoolean callbacksEnabled = new AtomicBoolean(true);
        try {
            Class<?> minecraft = Class.forName("net.minecraft.client.Minecraft", true, loader);
            client = minecraft.getMethod("getInstance").invoke(null);
            if (client == null) throw new IllegalStateException("the Horizon Minecraft facade returned no client");
            Class<?> events = Class.forName("net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents", true, loader);
            Object startEvent = events.getField("START_CLIENT_TICK").get(null);
            startInvoker = startEvent.getClass().getMethod("invoker").invoke(startEvent);
            startCallback = findTickCallback(startInvoker, "onStartTick");
            Object endEvent = events.getField("END_CLIENT_TICK").get(null);
            endInvoker = endEvent.getClass().getMethod("invoker").invoke(endEvent);
            endCallback = findTickCallback(endInvoker, "onEndTick");
            System.err.println("Horizon Fabric bridge: client ticks, player/environment and nearby-block snapshots, vanilla movement input, keyboard key mappings, bounded sprint/velocity/look actions, system chat, and action-bar text are mapped to Rust Horizon.");
        } catch (ReflectiveOperationException | RuntimeException error) {
            System.err.println("Horizon Fabric bridge: a mapped Fabric client-tick event is unavailable: " + error);
        }

        long callbacks = 0;
        String line;
        try {
            while ((line = input.readLine()) != null) {
                if ("stop".equals(line)) break;
                String[] fields = line.split("\\t", -1);
                if (fields.length == 0) continue;
                boolean start = "start".equals(fields[0]);
                boolean end = "end".equals(fields[0]);
                if (!start && !end) continue;
                try {
                    updateTickSnapshot(client, fields);
                } catch (IllegalArgumentException | CharacterCodingException error) {
                    callbacksEnabled.set(false);
                    System.err.println("Horizon Fabric bridge: invalid Rust game/input snapshot; callbacks stopped: " + error.getMessage());
                    output.println("error");
                    continue;
                }
                Object invoker = start ? startInvoker : endInvoker;
                Method callback = start ? startCallback : endCallback;
                if (callback == null || invoker == null) {
                    callbacksEnabled.set(false);
                    output.println("unsupported");
                    continue;
                }
                if (callbacksEnabled.get()) {
                    try {
                        callback.invoke(invoker, client);
                    } catch (InvocationTargetException error) {
                        callbacksEnabled.set(false);
                        System.err.println("Horizon Fabric bridge: END_CLIENT_TICK stopped after a mod callback failed: " + error.getCause());
                        output.println("error");
                        continue;
                    } catch (ReflectiveOperationException | RuntimeException error) {
                        callbacksEnabled.set(false);
                        System.err.println("Horizon Fabric bridge: END_CLIENT_TICK stopped after a mod callback failed: " + error);
                        output.println("error");
                        continue;
                    }
                }
                callbacks++;
                output.println("tick\t" + callbacks + "\t" + takeOverlayAction() + "\t"
                        + takeSprintingAction() + "\t" + takePlayerVelocityAction() + "\t"
                        + takeSystemMessageAction() + "\t" + takePlayerRotationAction());
            }
        } catch (Exception error) {
            System.err.println("Horizon Fabric bridge: stopped reading Horizon ticks: " + error.getMessage());
        }
    }

    private static Method findTickCallback(Object invoker, String callbackName) {
        Method callback = Arrays.stream(invoker.getClass().getMethods())
                .filter(method -> method.getName().equals(callbackName) && method.getParameterCount() == 1)
                .findFirst()
                .orElseThrow(() -> new IllegalStateException("Fabric " + callbackName + " invoker was not found"));
        callback.setAccessible(true);
        return callback;
    }

    private static void updateTickSnapshot(Object client, String[] fields)
            throws CharacterCodingException {
        net.minecraft.client.Minecraft minecraft = (net.minecraft.client.Minecraft) client;
        final int nativeInputField;
        final int downKeysField;
        final int pressedKeysField;
        if (fields.length == 7 && "-".equals(fields[1])) {
            if (!"-".equals(fields[6])) throw new IllegalArgumentException("empty-player tick carried block samples");
            minecraft.updateSnapshot(null, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    false, false, parseOptionalLong(fields[2]), 0, 0, 0,
                    false, false, 0, 0, 0, 0, fields[6]);
            nativeInputField = 3;
            downKeysField = 4;
            pressedKeysField = 5;
        } else {
            if (fields.length != 27 || "-".equals(fields[1])) {
                throw new IllegalArgumentException("unexpected tick snapshot field count");
            }
            if ((fields[1].length() & 1) != 0 || fields[1].length() > 256) {
                throw new IllegalArgumentException("player name exceeds the bridge limit");
            }
            byte[] nameBytes;
            try {
                nameBytes = HexFormat.of().parseHex(fields[1]);
            } catch (IllegalArgumentException error) {
                throw new IllegalArgumentException("player name is not valid hex", error);
            }
            String name = StandardCharsets.UTF_8.newDecoder()
                    .onMalformedInput(CodingErrorAction.REPORT)
                    .onUnmappableCharacter(CodingErrorAction.REPORT)
                    .decode(ByteBuffer.wrap(nameBytes)).toString();
            if (name.isEmpty() || name.codePoints().anyMatch(Character::isISOControl)) {
                throw new IllegalArgumentException("player name is empty or contains control characters");
            }
            double x = Double.parseDouble(fields[2]);
            double y = Double.parseDouble(fields[3]);
            double z = Double.parseDouble(fields[4]);
            double velocityX = Double.parseDouble(fields[5]);
            double velocityY = Double.parseDouble(fields[6]);
            double velocityZ = Double.parseDouble(fields[7]);
            float yRot = Float.parseFloat(fields[8]);
            float xRot = Float.parseFloat(fields[9]);
            float health = Float.parseFloat(fields[10]);
            long gameTime = Long.parseLong(fields[14]);
            int minBuildHeight = Integer.parseInt(fields[15]);
            int maxBuildHeight = Integer.parseInt(fields[16]);
            float rainLevelPrevious = Float.parseFloat(fields[19]);
            float rainLevelCurrent = Float.parseFloat(fields[20]);
            float thunderLevelPrevious = Float.parseFloat(fields[21]);
            float thunderLevelCurrent = Float.parseFloat(fields[22]);
            if (!Double.isFinite(x) || !Double.isFinite(y) || !Double.isFinite(z)
                    || !Double.isFinite(velocityX) || !Double.isFinite(velocityY)
                    || !Double.isFinite(velocityZ)
                    || !Float.isFinite(yRot) || !Float.isFinite(xRot) || !Float.isFinite(health)) {
                throw new IllegalArgumentException("player snapshot contains a non-finite number");
            }
            if (minBuildHeight < -4096 || maxBuildHeight > 4096
                    || minBuildHeight >= maxBuildHeight) {
                throw new IllegalArgumentException("player snapshot contains invalid build-height bounds");
            }
            if (!validLevel(rainLevelPrevious) || !validLevel(rainLevelCurrent)
                    || !validLevel(thunderLevelPrevious) || !validLevel(thunderLevelCurrent)) {
                throw new IllegalArgumentException("player snapshot contains invalid rain or thunder levels");
            }
            boolean sprinting = parseBoolean(fields[11]);
            boolean onGround = parseBoolean(fields[12]);
            boolean raining = parseBoolean(fields[17]);
            boolean thundering = parseBoolean(fields[18]);
            minecraft.updateSnapshot(name, x, y, z, velocityX, velocityY, velocityZ,
                    yRot, xRot, health, sprinting, onGround, parseOptionalLong(fields[13]),
                    gameTime, minBuildHeight, maxBuildHeight, raining, thundering,
                    rainLevelPrevious, rainLevelCurrent, thunderLevelPrevious, thunderLevelCurrent,
                    fields[26]);
            nativeInputField = 23;
            downKeysField = 24;
            pressedKeysField = 25;
        }
        net.minecraft.client.KeyMapping.updateBridgeInput(
                parseKeyCodes(fields[downKeysField], true), parseKeyCodes(fields[pressedKeysField], false),
                parseNativeInputFlags(fields[nativeInputField]));
    }

    private static int parseNativeInputFlags(String value) {
        final int flags;
        try {
            flags = Integer.parseInt(value);
        } catch (NumberFormatException error) {
            throw new IllegalArgumentException("native movement input flags are invalid", error);
        }
        if (flags < 0 || (flags & ~0x7f) != 0) {
            throw new IllegalArgumentException("native movement input flags are outside the mapped controls");
        }
        return flags;
    }

    private static List<Integer> parseKeyCodes(String value, boolean unique) {
        if ("-".equals(value)) return List.of();
        String[] values = value.split(",", -1);
        if (values.length > 64) throw new IllegalArgumentException("too many key codes in one tick");
        List<Integer> codes = new ArrayList<>(values.length);
        Set<Integer> seen = unique ? new HashSet<>() : null;
        for (String item : values) {
            final int code;
            try {
                code = Integer.parseInt(item);
            } catch (NumberFormatException error) {
                throw new IllegalArgumentException("keyboard key code is invalid", error);
            }
            if (code < 0 || code > 255) throw new IllegalArgumentException("Minecraft 26.3 keyboard key code is out of range");
            if (unique && !seen.add(code)) throw new IllegalArgumentException("held keyboard key is duplicated");
            codes.add(code);
        }
        return codes;
    }

    private static Long parseOptionalLong(String value) {
        if ("-".equals(value)) return null;
        return Long.parseLong(value);
    }

    private static boolean parseBoolean(String value) {
        if ("true".equals(value)) return true;
        if ("false".equals(value)) return false;
        throw new IllegalArgumentException("snapshot boolean is invalid");
    }

    private static boolean validLevel(float value) {
        return Float.isFinite(value) && value >= 0.0f && value <= 1.0f;
    }

}
