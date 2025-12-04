package io.github.treesitter.ktreesitter.plugin;

import java.util.Map;

import org.gradle.api.NonNullApi;
import org.gradle.api.Plugin;
import org.gradle.api.Project;

/**
 * A plugin that generates code for
 * <a href="https://tree-sitter.github.io/kotlin-tree-sitter/">KTreeSitter</a>
 * grammar packages.
 */
@NonNullApi
public abstract class GrammarPlugin implements Plugin<Project> {
    @Override
    public void apply(Project project) {
        var extension = project.getExtensions().create("grammar", GrammarExtension.class);
        extension.getInteropName().convention("grammar");
        extension.getBaseDir().convention(
            project.getProjectDir().getParentFile().getParentFile()
        );
        extension.getLibraryName().convention(
            extension.getGrammarName().map(name -> "ktreesitter-" + name)
        );
        extension.getLanguageMethods().convention(
            extension.getGrammarName().map(name -> Map.of("language", "tree_sitter_" + name))
        );

        project.getTasks().register("generateGrammarFiles", GrammarFilesTask.class, it -> {
            it.setGrammarDir(extension.getBaseDir().get());
            it.setGrammarName(extension.getGrammarName().get());
            it.setInteropName(extension.getInteropName().get());
            it.setLibraryName(extension.getLibraryName().get());
            it.setPackageName(extension.getPackageName().get());
            it.setClassName(extension.getClassName().get());
            it.setLanguageMethods(extension.getLanguageMethods().get());

            var generatedDir = project.getLayout().getBuildDirectory().get().dir("generated");
            it.getGeneratedSrc().set(generatedDir.dir("src"));
            it.getCmakeListsFile().set(generatedDir.file("CMakeLists.txt"));
            it.getInteropFile().set(
                generatedDir.dir("src").dir("nativeInterop").file(it.getInteropName() + ".def")
            );

            it.getOutputs().dir(it.getGeneratedSrc());
            it.getOutputs().files(it.getCmakeListsFile(), it.getInteropFile());
        });
    }
}
