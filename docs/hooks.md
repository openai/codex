# Hooks 莉墓ｧ假ｼ医ラ繝ｩ繝輔ヨ・・

## 逶ｮ逧・
- TurnStart / TurnEnd 縺ｮ繧ｿ繧､繝溘Φ繧ｰ縺ｧ繧ｳ繝槭Φ繝峨ｒ閾ｪ蜍募ｮ溯｡後☆繧倶ｻ慕ｵ・∩繧呈署萓帙☆繧九・
- Hooks 閾ｪ菴薙・縲後さ繝槭Φ繝牙ｮ溯｡後ｒ莉慕ｵ・喧縺吶ｋ縺縺代阪〒縺ゅｊ縲∝ｮ溯｡檎ｵ先棡縺ｮ謇ｱ縺・ｼ医お繝ｼ繧ｸ繧ｧ繝ｳ繝医∈縺ｮ謖・､ｺ蛹悶∵ｬ｡Turn襍ｷ蜍輔↑縺ｩ・峨・ Codex 蛛ｴ縺ｮ蜃ｦ逅・Ν繝ｼ繝ｫ縺ｫ蠕薙≧縲・

## 逕ｨ隱・
- TurnStartHook: Turn髢句ｧ狗峩蜑阪↓螳溯｡後＆繧後ｋ hook
- TurnEndHook: Turn邨ゆｺ・峩蠕後↓螳溯｡後＆繧後ｋ hook
- HookInput: Hook 縺ｮ stderr 繧・Codex 蛛ｴ縺ｸ貂｡縺吶◆繧√・蜈･蜉帛ｽ｢蠑・

## 險ｭ螳壹ヵ繧｡繧､繝ｫ

### 繝代せ
- <workspace>/.agent/hook.toml

### 蠖｢蠑擾ｼ・VP・・
```toml
[turn_start]
command = "path/to/script"
args = ["--foo", "bar"]
run_on_hook_input = true

[turn_end]
command = "path/to/script"
args = ["--baz"]
```

### 莉墓ｧ・
- command: 螳溯｡後☆繧九さ繝槭Φ繝・
- args: 蠑墓焚驟榊・・育怐逡･蜿ｯ・・
- run_on_hook_input: (turn_start only) HookInput 由来の Turn でも TurnStartHook を実行するか (default true)
- cwd: 繝ｯ繝ｼ繧ｯ繧ｹ繝壹・繧ｹ蝗ｺ螳・
- 繧ｿ繧､繝繧｢繧ｦ繝医↑縺暦ｼ・VP・・
- 螟ｱ謨励＠縺ｦ繧ゅそ繝・す繝ｧ繝ｳ繧呈ｭ｢繧√↑縺・

## 螳溯｡御ｻ墓ｧ・

### TurnStartHook
1. Turn髢句ｧ句燕縺ｫ hook 螳溯｡・
2. stdout/stderr 繧・ExecCommandBegin/End 縺ｨ StdoutStream 邨檎罰縺ｧ UI 縺ｸ騾夂衍・・serShellCommand 縺ｨ蜷後§繝ｪ繧｢繝ｫ繧ｿ繧､繝陦ｨ遉ｺ・・
3. stderr 縺碁撼遨ｺ縺ｪ繧・HookInput 縺ｨ縺励※蜷後§ Turn 縺ｫ霑ｽ蜉 竊・繝ｦ繝ｼ繧ｶ繝ｼ蜈･蜉帙→蜷域・縺励※騾∽ｿ｡

### TurnEndHook
1. Turn螳御ｺ・峩蠕後↓ hook 螳溯｡・
2. stdout/stderr 繧・ExecCommandBegin/End 縺ｨ StdoutStream 邨檎罰縺ｧ UI 縺ｸ騾夂衍・・serShellCommand 縺ｨ蜷後§繝ｪ繧｢繝ｫ繧ｿ繧､繝陦ｨ遉ｺ・・
3. stderr 縺碁撼遨ｺ縺ｪ繧・HookInput 縺ｨ縺励※騾∽ｿ｡縺励∵眠縺励＞ Turn 繧帝幕蟋・

### Sandbox / Approval
- Turn 縺ｮ sandbox / approval 險ｭ螳壹↓蠕薙≧

### 繧ｭ繝｣繝ｳ繧ｻ繝ｫ
- Esc 縺ｫ繧医ｋ Op::Interrupt 縺ｨ蜷後§邨瑚ｷｯ縺ｧ荳ｭ譁ｭ蜿ｯ閭ｽ

## UI 陦ｨ遉ｺ

### Hook 螳溯｡後Ο繧ｰ
- UserShellCommand 縺ｨ蜷檎ｭ峨・隕九○譁ｹ
- 繧ｿ繧､繝医Ν陦ｨ遉ｺ萓・
  - Hook(TurnStart)
  - Hook(TurnEnd)
- ExecCommandBegin/End 縺ｨ stdout/stderr 繧定｡ｨ遉ｺ

### HookInput 陦ｨ遉ｺ
- HookInput 縺ｧ縺ゅｋ縺薙→縺梧・遉ｺ逧・↓蛻・°繧玖｡ｨ遉ｺ
- 陦ｨ遉ｺ萓・
  - HookInput: <stderr text>
- history 縺ｫ縺ｯ谿九＆縺ｪ縺・

## 螻･豁ｴ繝ｻ險倬鹸
- history: 谿九＆縺ｪ縺・
- rollout: 谿九☆
  - Hook 螳溯｡後Ο繧ｰ・・xecCommandBegin/End・・
  - HookInput

## HookInput 莉墓ｧ・

### 逶ｮ逧・
- Hook 縺ｮ stderr 繧偵後ヵ繝・け逕ｱ譚･縺ｮ蜈･蜉帙阪→縺励※ Codex 縺ｫ貂｡縺・
- 繝ｦ繝ｼ繧ｶ繝ｼ蜈･蜉帙→縺ｯ蛹ｺ蛻･縺吶ｋ

### 螳溯｣・｡・
- protocol 縺ｫ HookInput 讒矩菴薙ｒ霑ｽ蜉
- Op::HookInput 繧定ｿｽ蜉
- HookInput 縺ｯ UI 陦ｨ遉ｺ繝ｻrollout險倬鹸縺ｮ蟇ｾ雎｡縺縺後”istory縺ｫ縺ｯ谿九＆縺ｪ縺・

## 豕ｨ諢冗せ繝ｻ險ｭ險育炊逕ｱ
- stderr 縺ｯ繝励Ο繧ｻ繧ｹ蜊倅ｽ阪〒蛻・屬縺輔ｌ繧九◆繧√？ook 螳溯｡後・ stderr 縺御ｻ悶・繧ｳ繝槭Φ繝牙・蜉帙→豺ｷ縺悶ｋ縺薙→縺ｯ縺ｪ縺・・
- Hook 縺ｮ stderr 蜀・ｮｹ縺ｮ蛻ｶ蠕｡縺ｯ hook 繧ｹ繧ｯ繝ｪ繝励ヨ菴懆・・雋ｬ莉ｻ縺ｨ縺吶ｋ縲・

## 髱樒岼讓呻ｼ・VP・・
- 繧ｿ繧､繝繧｢繧ｦ繝亥宛蠕｡
- 髱槫酔譛溷ｮ溯｡・
- Hook 邨先棡縺ｮJSON繝励Ο繝医さ繝ｫ
- Hook 邨先棡縺ｫ繧医ｋ謇ｿ隱・荳ｭ譁ｭ縺ｮ閾ｪ蜍募喧

## 繝・せ繝域婿驥晢ｼ・VP・・
- core 繝ｬ繧､繝､繝ｼ縺ｮ繝・せ繝医〒謖吝虚繧剃ｿ晞囿縺吶ｋ・・urnStart/TurnEnd 縺ｮ螳溯｡後→ HookInput 縺ｮ豕ｨ蜈･・・
- tui/cli 縺ｮ陦ｨ遉ｺ菫晁ｨｼ縺ｯ迴ｾ谿ｵ髫弱〒縺ｯ蟇ｾ雎｡螟・

## 霑ｽ蜉讀懆ｨ趣ｼ亥ｰ・擂諡｡蠑ｵ・・
- HookInput 縺ｮ JSON 蠖｢蠑・
- Hook 螳溯｡後・繧ｿ繧､繝繧｢繧ｦ繝・
- Hook 謌仙凄縺ｫ蠢懊§縺溯・蜍墓嫌蜍包ｼ井ｾ・ TurnEnd縺ｧ閾ｪ蜍募●豁｢・・

