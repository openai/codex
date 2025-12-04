import java.util.Properties

group = "io.github.tree-sitter"
version = with(Properties()) {
    file("../gradle.properties").reader().use(::load)
    getProperty("project.version")
}

plugins {
    `java-gradle-plugin`
    id("com.gradle.plugin-publish") version "1.3.1"
}

repositories {
    mavenCentral()
}

java {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
}

gradlePlugin {
    vcsUrl = "https://github.com/tree-sitter/kotlin-tree-sitter"
    website = "https://github.com/tree-sitter/kotlin-tree-sitter/tree/master/ktreesitter-plugin"
    plugins.create("ktreesitter") {
        id = "$group.${project.name}"
        displayName = "KTreeSitter grammar plugin"
        description = "A plugin that generates code for KTreeSitter grammar packages"
        implementationClass = "io.github.treesitter.ktreesitter.plugin.GrammarPlugin"
        tags = listOf("tree-sitter", "code", "generator")
    }
}

tasks.javadoc {
    (options as CoreJavadocOptions).addBooleanOption("Xdoclint:all,-missing", true)
}

tasks.validatePlugins {
    enableStricterValidation.set(true)
}
