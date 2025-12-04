package io.github.treesitter.ktreesitter.plugin;

import java.io.File;
import java.io.FileWriter;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.regex.Pattern;

import org.gradle.api.DefaultTask;
import org.gradle.api.GradleException;
import org.gradle.api.NonNullApi;
import org.gradle.api.file.DirectoryProperty;
import org.gradle.api.file.RegularFileProperty;
import org.gradle.api.tasks.*;

/** The task that generates the source files for the grammar. */
@NonNullApi
@CacheableTask
public abstract class GrammarFilesTask extends DefaultTask {
    private File grammarDir;

    private String grammarName;

    private String interopName;

    private String libraryName;

    private String packageName;

    private String className;

    private Map<String, String> languageMethods;

    private static final Pattern packageNameRegex =
        Pattern.compile("^[A-Za-z_]\\w*(?:[.][A-Za-z_]\\w*)*$");

    /** Get the base directory of the grammar. */
    @InputDirectory
    @PathSensitive(PathSensitivity.ABSOLUTE)
    public final File getGrammarDir() {
        return grammarDir;
    }

    /** Set the base directory of the grammar. */
    public final void setGrammarDir(File grammarDir) {
        this.grammarDir = grammarDir;
    }

    /** Get the name of the grammar. */
    @Input
    public final String getGrammarName() {
        return grammarName;
    }

    /** Set the name of the grammar. */
    public final void setGrammarName(String grammarName) {
        this.grammarName = grammarName;
    }

    /** Get the name of the C interop def file. */
    @Input
    public final String getInteropName() {
        return interopName;
    }

    /** Set the name of the C interop def file. */
    public final void setInteropName(String interopName) {
        this.interopName = interopName;
    }

    /** Get the name of the JNI library. */
    @Input
    public final String getLibraryName() {
        return libraryName;
    }

    /** Set the name of the JNI library. */
    public final void setLibraryName(String libraryName) {
        this.libraryName = libraryName;
    }

    /** Get the name of the package. */
    @Input
    public final String getPackageName() {
        return packageName;
    }

    /** Set the name of the package. */
    public final void setPackageName(String packageName) throws GradleException {
        if (!packageNameRegex.matcher(packageName).matches())
            throw new GradleException("Package name is not valid: " + packageName);
        this.packageName = packageName;
    }

    /** Get the name of the class. */
    @Input
    public final String getClassName() {
        return className;
    }

    /** Set the name of the class. */
    public final void setClassName(String className) throws GradleException {
        if (invalidIdentifier(className))
            throw new GradleException("Class name is not valid: " + className);
        this.className = className;
    }

    /** Get the language methods. */
    @Input
    public final Map<String, String> getLanguageMethods() {
        return languageMethods;
    }

    /** Set the language methods. */
    public final void setLanguageMethods(
        Map<String, String> languageMethods
    ) throws GradleException {
        for (var method : languageMethods.entrySet()) {
            if (invalidIdentifier(method.getKey()))
                throw new GradleException("Method name is not valid: " + method.getKey());
            if (invalidIdentifier(method.getValue()))
                throw new GradleException("Method name is not valid: " + method.getValue());
        }
        this.languageMethods = languageMethods;
    }

    /** Get the directory of the generated files. */
    @OutputDirectory
    public abstract DirectoryProperty getGeneratedSrc();

    /** Get the generated {@code CMakeLists.txt} file. */
    @OutputFile
    public abstract RegularFileProperty getCmakeListsFile();

    /** Get the generated C interop def file. */
    @OutputFile
    public abstract RegularFileProperty getInteropFile();

    /** Generate the output files. */
    @TaskAction
    public final void generate() throws GradleException {
        var srcDir = getGeneratedSrc().get().getAsFile();
        mkdirs(srcDir);

        var srcPath = srcDir.toPath();
        generateCommon(srcPath);
        generateNative(srcPath);
        generateJvm(srcPath);
        generateAndroid(srcPath);
        generateJniBinding(srcPath);
        generateCmakeLists(srcPath);
        generateInterop();
    }

    private void generateCommon(Path srcDir) throws GradleException {
        var classFile = srcDir.resolve(
            "commonMain/kotlin/%s/%s.kt".formatted(
                packageName.replace('.', '/'), className
            )
        );
        mkdirs(classFile.getParent().toFile());

        var methods = languageMethods.keySet().stream().map("    fun %s(): Any"::formatted);
        var template = readResource("common.kt.in")
            .replace("@PACKAGE@", packageName)
            .replace("@CLASS@", className)
            .replace("@METHODS@", String.join("\n\n", methods.toList()));
        writeFile(classFile.toFile(), template);
    }

    private void generateNative(Path srcDir) throws GradleException {
        var classFile = srcDir.resolve(
            "nativeMain/kotlin/%s/%s.kt".formatted(
                packageName.replace('.', '/'), className
            )
        );
        mkdirs(classFile.getParent().toFile());

        var imports = languageMethods.values().stream().map(method ->
            "import %s.internal.%s".formatted(packageName, method)
        );
        var methods = languageMethods.entrySet().stream().map(method ->
            "    actual fun %s(): Any = %s()!!".formatted(method.getKey(), method.getValue())
        );
        var template = readResource("native.kt.in")
            .replace("@PACKAGE@", packageName)
            .replace("@CLASS@", className)
            .replace("@IMPORTS@", String.join("\n\n", imports.toList()))
            .replace("@METHODS@", String.join("\n\n", methods.toList()));
        writeFile(classFile.toFile(), template);
    }

    private void generateJvm(Path srcDir) throws GradleException {
        var classFile = srcDir.resolve(
            "jvmMain/kotlin/%s/%s.kt".formatted(
                packageName.replace('.', '/'), className
            )
        );
        mkdirs(classFile.getParent().toFile());

        var methods = languageMethods.entrySet().stream().map(method ->
            """
                    actual fun %s(): Any = %s()

                    @JvmStatic
                    private external fun %s(): Long
                """.stripIndent().formatted(
                method.getKey(), method.getValue(), method.getValue()
            )
        );
        var template = readResource("jvm.kt.in")
            .replace("@PACKAGE@", packageName)
            .replace("@CLASS@", className)
            .replace("@LIBRARY@", libraryName)
            .replace("@METHODS@", String.join("\n\n", methods.toList()));
        writeFile(classFile.toFile(), template);
    }

    private void generateAndroid(Path srcDir) throws GradleException {
        var classFile = srcDir.resolve(
            "androidMain/kotlin/%s/%s.kt".formatted(
                packageName.replace('.', '/'), className
            )
        );
        mkdirs(classFile.getParent().toFile());

        var methods = languageMethods.entrySet().stream().map(method ->
                """
                        actual fun %s(): Any = %s()

                        @JvmStatic
                        @CriticalNative
                        private external fun %s(): Long
                    """.stripIndent().formatted(
                        method.getKey(), method.getValue(), method.getValue()
                )
        );
        var template = readResource("android.kt.in")
            .replace("@PACKAGE@", packageName)
            .replace("@CLASS@", className)
            .replace("@LIBRARY@", libraryName)
            .replace("@METHODS@", String.join("\n\n", methods.toList()));
        writeFile(classFile.toFile(), template);
    }

    private void generateJniBinding(Path srcDir) throws GradleException {
        var jniBinding = srcDir.resolve("jni").resolve("binding.c");
        mkdirs(jniBinding.getParent().toFile());

        var jniClassName = jniTransform(className);
        var jniPackageName = jniTransform(packageName).replace('.', '_');
        var jniPrefix = "Java_%s_%s_".formatted(jniPackageName, jniClassName);
        var methods = languageMethods.values().stream().map(name ->
            "NATIVE_FUNCTION(%s%s) {\n    return (jlong)%s();\n}".formatted(
                jniPrefix, jniTransform(name), name
            )
        );
        var template = readResource("jni.c.in")
            .replace("@GRAMMAR@", grammarName)
            .replace("@FUNCTIONS@", String.join("", methods.toList()));
        writeFile(jniBinding.toFile(), template);
    }

    private void generateCmakeLists(Path srcDir) throws GradleException {
        var jniBinding = srcDir.resolve("jni").resolve("binding.c");
        var includeDir = relative(grammarDir.toPath().resolve("bindings/c"));
        var sources = String.format("%s %s", relative(jniBinding), srcFiles());
        var template = readResource("CMakeLists.txt.in")
            .replace("@LIBRARY@", libraryName)
            .replace("@INCLUDE@", includeDir.toString())
            .replace("@SOURCES@", sources);
        writeFile(getCmakeListsFile().get().getAsFile(), template);
    }

    private void generateInterop() throws GradleException {
        var interopFile = getInteropFile().get().getAsFile();
        mkdirs(interopFile.getParentFile());

        var template = readResource("interop.def.in")
            .replace("@PACKAGE@", packageName)
            .replace("@GRAMMAR@", grammarName);
        writeFile(interopFile, template);
    }

    private Path relative(Path file) {
        return getGeneratedSrc().get().getAsFile().toPath().getParent().relativize(file);
    }

    private String srcFiles() {
        var grammarSrcDir = grammarDir.toPath().resolve("src");
        var scannerFile = grammarSrcDir.resolve("scanner.c");
        if (!scannerFile.toFile().exists()) {
            return grammarSrcDir.resolve("parser.c").toString();
        }
        return String.format("%s %s", grammarSrcDir.resolve("parser.c"), scannerFile);
    }

    private String readResource(String file) throws GradleException {
        try (var stream = getClass().getResourceAsStream("/" + file)) {
            var bytes = Objects.requireNonNull(stream).readAllBytes();
            return new String(bytes, StandardCharsets.UTF_8);
        } catch (IOException | NullPointerException ex) {
            throw new GradleException("Failed to read resource file: " + file, ex);
        }
    }

    private static void writeFile(File file, String content) throws GradleException {
        try (var writer = new FileWriter(file)) {
            writer.write(content);
        } catch (IOException e) {
            throw new GradleException("Failed to write to file: " + file, e);
        }
    }

    private static String jniTransform(String input) {
        var builder = new StringBuilder();
        for (char c : input.toCharArray()) {
            switch (c) {
                case '_':
                    builder.append("_1");
                    break;
                case ';':
                    builder.append("_2");
                    break;
                case '[':
                    builder.append("_3");
                    break;
                default:
                    builder.append(c <= 0x7F ? c : "_0%04x".formatted((int)c));
            }
        }
        return builder.toString();
    }

    private static boolean invalidIdentifier(String input) {
        if (!Character.isJavaIdentifierStart(input.charAt(0))) return true;
        return !input.substring(1).chars().allMatch(Character::isJavaIdentifierPart);
    }

    @SuppressWarnings("ResultOfMethodCallIgnored")
    private static void mkdirs(File file) { file.mkdirs(); }
}
