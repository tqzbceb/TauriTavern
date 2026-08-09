# 上游 issue 文本(给 Darkatse / TauriTavern 作者)

> 用途:把以下文本贴到 https://github.com/Darkatse/TauriTavern/issues 作为提建议。
> 不发 PR,不提 fork 已改的事实,避免让作者觉得被绕过。

---

**标题**:更新检查在多次重启后命中 GitHub 匿名速率限制,误报 toast 影响装扩展体验

**现象**:
在 TauriTavern 安卓 1.6.x 版上,反复装/启扩展(每次都 reload)之后再操作一会儿,常常弹出 toast:

> 你的请求已触发 GitHub 的速率限制。请稍后再试,或更换IP后重试。

换 IP 节点能恢复一会儿,继续装几个扩展又弹。同网络环境下电脑版 ST 不弹。

**根因**:
`tauritavern-version` 扩展在 `APP_READY` 时无条件触发 `runStartupUpdateCheck`
(`src/scripts/extensions/tauritavern-version/index.js:459`),
它调 `checkForUpdate` → `GitHubUpdateRepository::get_latest_release`
(`src-tauri/src/infrastructure/apis/github_update_repository.rs:36`),
后者不带 Authorization 头直接打
`https://api.github.com/repos/Darkatse/TauriTavern/releases/latest`,走的是
GitHub 匿名池 60 次/小时(按出口 IP 计)。

「装/启扩展 → location.reload → APP_READY → update check」这条链让用户每装
一个就扣一次匿名额度,1 小时内装 4~5 个扩展的用户很容易撞 60/h 顶,弹
「速率限制」toast。

更新检查的 toast 文案确实命中 `classify_github_rate_limit` 的双条件判别
(`src-tauri/src/infrastructure/github.rs:22`,要 status 是 403/429 且 body 含
"rate limit" / "abuse detection"),不是误报——是真撞墙了。

**建议**:
1. 给 `GitHubUpdateRepository::get_latest_release` 加可选 `Authorization: Bearer <PAT>` 头,
   从用户在设置里填的 PAT 取(可复用已有 `SecretKeys` 子系统,
   新增 `SecretKeys::GITHUB_TOKEN = "github_token"`)。把匿名 60/h 提升到认证 5000/h。
   PAT 缺/空时自动降级匿名调用,不抛错。
2. 「启动时自动检查更新」加一个 settings 开关,默认 false
   (用户主动想查时手动点「检查更新」按钮即可),从源头少扣额度。
3. 手动点「检查更新」也加一个本地内存缓存(30 分钟 TTL),避免用户连点 N 次扣 N 次额度。
   缓存只存内存,App 重启失效,符合用户预期。

附:若愿修,可以提供 patch 思路。
