rootProject.name = "anstyle-kotlin"

// Include sibling projects for anstyle-roff module
includeBuild("../roff-kotlin") {
    dependencySubstitution {
        substitute(module("io.github.kotlinmania:roff-kotlin")).using(project(":"))
    }
}

includeBuild("../cansi-kotlin") {
    dependencySubstitution {
        substitute(module("io.github.kotlinmania:cansi-kotlin")).using(project(":"))
    }
}
