› here is what i will be working on
  it will be multi agent suport for codex cli
  so we have main agent that user can interact with via cli
  but this agent will be able to talk to subagent that will have its own AGENTS.md
  and config.toml
  as well as
  /007   log/
  /008   sessions/

  so it will function exacly the same as ~/.codex dir in base codex
  but in ~/.codex it will be possible to create dir
  agents/
  that will store dirs of agents name of dir will be id of agent
  there there will be at least AGENTS.md and config.toml crutialy
  to set it up

  first we are in design phase
  in root of repo make dir
  ai-temp/

  there place AGENTS.md where we keep info about this feture specific
  to keep ai context while we are working on it


  first of all investigete how related moving parts are implemented in existing codex
  codebase and what we can reuse and how are we going to hook that in
  that will be first step

  than
  make AGENTS.md with your findign related to my feture idea than overall roadmap
  basic design principals archetecute so i can have a think about it
  referece relative paths of files that you reference so i can find it and have a look.

  some more points
  i want it to be decoupled form main codebase where possible so its easy to maintain and
  interfaces shuld allow for change in other parts of codebase without a lot of changes in my
  part
  so we use some lvl of abstraction but not Java enterprise lvl that would be to much lol
  just a lil decopeling

  go for it create design documentations so we can analise this idea together
