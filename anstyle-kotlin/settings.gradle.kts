rootProject.name = "anstyle-kotlin"

// Include sibling projects for anstyle-roff module
includeBuild("../roff-kotlin") {
    dependencySubstitution {
        substitute(module("ai.solace.tui:roff-kotlin")).using(project(":"))
    }
}

includeBuild("../cansi-kotlin") {
    dependencySubstitution {
        substitute(module("ai.solace.tui:cansi-kotlin")).using(project(":"))
    }
}
