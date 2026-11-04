package net.minecraft.resources;

import java.util.Objects;

/** Narrow immutable Identifier facade used by mapped Minecraft 26.3 APIs. */
public final class Identifier {
    private final String namespace;
    private final String path;

    private Identifier(String namespace, String path) {
        this.namespace = namespace;
        this.path = path;
    }

    public static Identifier fromNamespaceAndPath(String namespace, String path) {
        if (!validNamespace(namespace) || !validPath(path)) {
            throw new IllegalArgumentException("Invalid Minecraft identifier: " + namespace + ":" + path);
        }
        return new Identifier(namespace, path);
    }

    private static boolean validNamespace(String value) {
        return value != null && !value.isEmpty() && value.chars().allMatch(character ->
                character >= 'a' && character <= 'z'
                        || character >= '0' && character <= '9'
                        || character == '_' || character == '-' || character == '.');
    }

    private static boolean validPath(String value) {
        return value != null && !value.isEmpty() && value.chars().allMatch(character ->
                character >= 'a' && character <= 'z'
                        || character >= '0' && character <= '9'
                        || character == '_' || character == '-' || character == '.' || character == '/');
    }

    public String getNamespace() {
        return namespace;
    }

    public String getPath() {
        return path;
    }

    @Override
    public String toString() {
        return namespace + ":" + path;
    }

    @Override
    public boolean equals(Object other) {
        return other instanceof Identifier identifier
                && namespace.equals(identifier.namespace)
                && path.equals(identifier.path);
    }

    @Override
    public int hashCode() {
        return Objects.hash(namespace, path);
    }
}
