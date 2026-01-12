<div>
<img src="https://github.com/acunniffe/git-ai/raw/main/assets/docs/git-ai.png" align="right"
     alt="Git AI by acunniffe/git-ai" width="100" height="100" />

</div>
<div>
<h1 align="left"><b>git-ai</b></h1>
</div>
<p align="left">Track the AI Code in your repositories</p>

<video src="https://github.com/user-attachments/assets/68304ca6-b262-4638-9fb6-0a26f55c7986" muted loop controls autoplay></video>

## Quick Start

#### Mac, Linux, Windows (WSL)

```bash
curl -sSL https://usegitai.com/install.sh | bash
```

#### Windows (non-WSL)

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -Command "irm http://usegitai.com/install.ps1 | iex"
```

🎊 That's it! **No per-repo setup.** Once installed Git AI will work OOTB with any of these **Supported Agents**:

<img src="https://github.com/acunniffe/git-ai/raw/main/assets/docs/supported-agents.png" width="320" />

### Next step: **Just code and commit!**

Once installed, all your Coding Agents will call Git AI in the background and mark the code they generate AI-authored.

After you commit, `git-ai` adds a git note to track which lines were AI-authored and scores each commit:

<img src="https://github.com/acunniffe/git-ai/raw/main/assets/docs/graph.jpg" width="400" />

## ⚠️ Fork Policy: Telemetry and Uploads Disabled

**This is a privacy-focused fork of git-ai that disables all outbound network reporting by default.**

### What's disabled:
- ✅ **All telemetry** (Sentry, PostHog) - No analytics or error reports sent
- ✅ **All prompt uploads** (CAS API) - Prompts stay on your machine
- ✅ **Background network processes** - No data leaves your system

### What still works:
- ✅ **Full local functionality** - AI authorship tracking, statistics, and visualization
- ✅ **Git notes storage** - All data stored locally in your repository
- ✅ **Agent integrations** - All supported agents work normally
- ✅ **SQLite database** - Local prompt and metadata storage

### Technical details:
All outbound network operations are gated by the `outbound_network_reporting_disabled()` function in `src/config.rs`. This policy cannot be overridden through configuration. To re-enable telemetry or uploads, you must modify the source code and rebuild.

**Privacy guarantee**: This fork will never upload prompts, telemetry, or any repository data to external servers, regardless of login state, API keys, or configuration settings.

## Installing the Stats Bot (early access)

Aggregate `git-ai` data at the PR, developer, Repository and Organization levels:

- AI authorship breakdown for every Pull Request
- Measure % of code that is AI generated through the entire SDLC
- Compare accepted-rate for code written by each Agent + Model. 
- AI-Code Halflife (how durable is the AI code)
> [Get early access by chatting with the maintainers](https://calendly.com/acunniffe/meeting-with-git-ai-authors)

![alt](https://github.com/acunniffe/git-ai/raw/main/assets/docs/dashboard.png)

## Prompt Storage
By default Git AI stores prompt data locally only. To include prompts in git notes (authorship logs), set `prompt_storage` to `notes`:

```bash
git-ai config set prompt_storage notes
```

When using notes mode, you can exclude specific repositories from having prompt data included:

```bash
git-ai config set --add exclude_prompts_in_repositories https://github.com/private-org/*
git-ai config set --add exclude_prompts_in_repositories /path/to/private/repo
```

*or to exclude all repositories:*

```bash
git-ai config set --add exclude_prompts_in_repositories "*"
```

## Goals of `git-ai` project

🤖 **Track AI code in a Multi-Agent** world. Because developers get to choose their tools, engineering teams need a **vendor agnostic** way to track AI impact in their repos.

🎯 **Accurate attribution** from Laptop → Pull Request → Merged. Claude Code, Cursor and Copilot cannot track code after generation—Git AI follows it through the entire workflow.

🔄 **Support real-world git workflows** by making sure AI-Authorship annotations survive a `merge --squash`, `rebase`, `reset`, `cherry-pick` etc.

🔗 **Maintain link between prompts and code** - there is valuable context and requirements in team prompts—preserve them alongside code.

🚀 **Git-native + Fast** - `git-ai` is built on git plumbing commands. Negligible impact even in large repos (&lt;100ms). Tested in [Chromium](https://github.com/chromium/chromium).

## [Documentation](https://usegitai.com/docs)

- How Git AI Works and its Limitations [▶️ Video](https://www.youtube.com/watch?v=b_DZTC1PKHI) [🗺️ Diagram](https://usegitai.com/docs/how-git-ai-works)
- [Git AI Commands](https://usegitai.com/docs/reference)
- [Configuring Git AI for the enterprise](https://usegitai.com/docs/administration/enterprise-configuration)

## Agent Support

`git-ai` automatically sets up all supported agent hooks using the `git-ai install-hooks` command

| Agent/IDE                                                                                  | Authorship | Prompts |
| ------------------------------------------------------------------------------------------ | ---------- | ------- |
| Cursor &gt;1.7                                                                             | ✅         | ✅      |
| Claude Code                                                                                | ✅         | ✅      |
| GitHub Copilot in VSCode via Extension                                                     | ✅         | ✅      |
| Google Gemini CLI                                                                          | ✅         | ✅      |
| Continue CLI                                                                               | ✅         | ✅      |
| OpenCode                                                                                   | ✅         | ✅      |
| Atlassian RovoDev CLI                                                                      | ✅         | ✅      |
| AWS Kiro (in-progress)                                                                     | 🔄         | 🔄      |
| Continue VS Code/IntelliJ (in-progress)                                                    | 🔄         | 🔄      |
| Windsurf                                                                                   | 🔄         | 🔄      |
| Augment Code                                                                               | 🔄         | 🔄      |
| OpenAI Codex (waiting on [openai/codex #2109](https://github.com/openai/codex/issues/2109)) |            |         |
| Junie &amp; Jetbrains IDEs                                                                 |            |         |
| Ona                                                                                        |            |         |
| Sourcegraph Cody + Amp                                                                     |            |         |
| Google Antigravity                                                                         |            |         |

| _your agent here_                                                                          |            |         |

> **Building a Coding Agent?** [Add support for Git AI by following this guide](https://usegitai.com/docs/add-your-agent)
