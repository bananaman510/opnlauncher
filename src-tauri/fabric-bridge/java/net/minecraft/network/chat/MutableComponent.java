package net.minecraft.network.chat;

/** Plain literal component for the Horizon HUD bridge. Styling is unsupported. */
public final class MutableComponent implements Component {
    private final String value;

    MutableComponent(String value) {
        if (value == null) throw new IllegalArgumentException("component text cannot be null");
        StringBuilder clean = new StringBuilder();
        value.codePoints()
                .filter(codePoint -> !Character.isISOControl(codePoint))
                .limit(256)
                .forEach(clean::appendCodePoint);
        this.value = clean.toString();
    }

    @Override
    public String getString() {
        return value;
    }
}
