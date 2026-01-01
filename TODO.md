# TODO - Assistant CLI

## Règles de développement

**À chaque itération / PR:**
- [x] `cargo build` - vérifier que le code compile
- [x] `cargo test` - tous les tests passent
- [x] `cargo clippy` - pas de warnings
- [x] Coverage suffisant des nouvelles fonctionnalités

**Structure des tests:**
- Tests unitaires dans chaque module (`#[cfg(test)] mod tests`)
- Tests d'intégration avec `wiremock` pour les appels GitHub API
- Pas de tests automatisés pour le TUI (tests manuels)

**Checklist avant de passer à la tâche suivante:**
1. La fonctionnalité marche (test manuel)
2. Tests unitaires ajoutés pour la nouvelle logique
3. `cargo test` passe
4. `cargo build --release` compile sans erreur

---

## 1. Clôturer une issue

**Objectif:** Fermer/rouvrir une issue directement depuis le TUI.

**Implémentation:**

### 1.1 API GitHub
- [x] Ajouter `GitHubConfig::close_issue(number)` - PATCH avec `state: "closed"`
- [x] Ajouter `GitHubConfig::reopen_issue(number)` - PATCH avec `state: "open"`

### 1.2 Interface utilisateur
- [x] Touche `x` dans la vue détail pour fermer l'issue
- [x] Afficher une confirmation: "Close issue #123? (y/n)"
- [x] Après fermeture: retour à la liste, issue retirée ou marquée comme fermée
- [x] Touche `X` (shift) pour rouvrir une issue fermée

### 1.3 Feedback visuel
- [x] Message de statut "Issue #123 closed" affiché temporairement
- [x] Différencier visuellement les issues fermées dans la liste (grisé, barré)

### 1.4 Fichiers à modifier
- [x] `src/github.rs`: Nouvelles méthodes
- [x] `src/tui.rs`: Handler de touche, confirmation, mise à jour de la liste

### 1.5 Tests à écrire
- [x] Test `close_issue()` avec wiremock (mock PATCH /issues/{number})
- [x] Test `reopen_issue()` avec wiremock
- [x] Test erreur 404 (issue inexistante)
- [x] Test erreur 403 (permissions insuffisantes)

---

## 2. Commande `/list` avec recherche intégrée

**Objectif:** Commande générique `/list` qui ouvre le TUI avec possibilité de filtrer/rechercher.

**Implémentation:**

### 2.1 Nouvelle commande `/list`
```
/list                       # Liste toutes les issues ouvertes
/list bug                   # Filtre par label "bug"
/list bug,customer          # Filtre par labels "bug" ET "customer"
/list auth                  # Recherche "auth" dans titre/body (si pas un label connu)
/list bug auth              # Label "bug" + recherche "auth"
/list "error handling"      # Recherche avec espaces (entre guillemets)
/list --state=closed        # Issues fermées
/list --state=closed auth   # Issues fermées contenant "auth"
```

### 2.2 Parser les arguments
- [x] Créer un struct `ListOptions { labels: Vec<String>, state: IssueState, search: Option<String> }`
- [x] Parser la syntaxe: labels séparés par virgule, options `--key=value`
- [x] Supporter `--state=open|closed|all`
- [x] Détecter si un argument est un label connu (défini dans config) ou un mot-clé de recherche
- [x] Supporter les guillemets pour recherche multi-mots: `"mon texte"`

### 2.3 API GitHub Search
- [x] Utiliser `octocrab` list issues avec state parameter (search via local filter)
- [x] Ajouter state parameter à `GitHubConfig::list_issues()` dans `github.rs`

### 2.4 Mode recherche dans le TUI (après ouverture)
- [x] Touche `/` pour entrer en mode recherche (comme vim)
- [x] Ajouter un champ `search_query: Option<String>` dans `IssueBrowser`
- [x] Afficher un champ de saisie en bas de l'écran
- [x] Filtrer les issues en temps réel pendant la saisie
- [x] `Escape` pour annuler, `Enter` pour valider et rester filtré

### 2.5 Logique de filtrage local (dans le TUI)
- [x] Créer `fn matches_query(issue: &IssueSummary, query: &str) -> bool`
- [x] Recherche case-insensitive sur: `title`, `labels`
- [x] Stocker les issues originales dans `all_issues: Vec<IssueSummary>`
- [x] `issues` devient la vue filtrée

### 2.6 Affichage
- [x] Afficher le query actif dans le titre: `" Issues (filtered: 'bug') "`
- [x] Touche `c` (clear) pour réinitialiser le filtre

### 2.7 Rétrocompatibilité
- [x] Garder les commandes dynamiques existantes (`list_commands` dans config) comme aliases
- [x] `/bugs` reste un alias pour `/list Bug`

### 2.8 Fichiers à modifier
- [x] `src/main.rs`: Nouveau handler pour `/list`, parser d'options
- [x] `src/github.rs`: Modifier `list_issues()` pour `State::All`
- [x] `src/list.rs`: Nouveau module pour ListOptions
- [x] `src/tui.rs`: Mode recherche, champ de saisie, logique de filtrage

### 2.9 Tests à écrire
- [x] Test parser `/list` sans arguments
- [x] Test parser `/list bug` (label seul)
- [x] Test parser `/list bug,feature` (multi-labels)
- [x] Test parser `/list auth` (recherche mot-clé)
- [x] Test parser `/list bug auth` (label + recherche)
- [x] Test parser `/list "error handling"` (guillemets)
- [x] Test parser `/list --state=closed`
- [x] Test `matches_query()` pour filtrage local (dans main.rs)

---

## 3. Infinite Scrolling pour la liste des issues

**Objectif:** Charger plus de 20 issues avec pagination automatique.

**Implémentation:**

### 3.1 Modifier `github.rs` pour supporter la pagination
- [x] Ajouter une méthode `list_issues_paginated()` qui retourne la page + info de pagination
- [x] Utiliser `octocrab::Page::next()` pour récupérer les pages suivantes
- [x] Retourne `(Vec<IssueSummary>, has_next_page: bool)`

### 3.2 Modifier `tui.rs` pour le chargement dynamique
- [x] Ajouter `has_next_page`, `current_page`, `is_loading` dans `IssueBrowser`
- [x] Détecter quand l'utilisateur atteint la fin de la liste (index >= issues.len() - 5)
- [x] Déclencher le chargement de la page suivante en async
- [x] Afficher un indicateur "[Loading...]" dans le titre
- [x] Fusionner les nouvelles issues dans `issues: Vec<IssueSummary>`

### 3.3 Fichiers à modifier
- [x] `src/github.rs`: Nouvelle méthode de pagination
- [x] `src/tui.rs`: Logique de détection et chargement
- [x] `src/main.rs`: Passer les infos de pagination au TUI

### 3.4 Tests à écrire
- [x] Test pagination avec wiremock (mock Link header)
- [x] Test `has_next_page` parsing
- [x] Test fusion des pages (pas de doublons)

---

## 4. Attribution de personnes sur les issues

**Objectif:** Assigner des utilisateurs à une issue avec auto-complete.

**Implémentation:**

### 4.1 API GitHub pour les assignees
- [x] Ajouter `GitHubConfig::list_assignees()` - liste les collaborateurs du repo
- [x] Ajouter `GitHubConfig::assign_issue(number, assignees)`
- [x] Ajouter `GitHubConfig::unassign_issue(number, assignees)`
- [x] Cache la liste des assignees au démarrage du TUI

### 4.2 Interface utilisateur dans le TUI
- [x] Touche `a` dans la vue détail pour ouvrir le sélecteur d'assignees
- [x] Nouvelle vue `TuiView::AssignUser { issue, input, suggestions, selected }`
- [x] Filtrage en temps réel pendant la saisie (fuzzy matching)
- [x] Navigation avec flèches haut/bas dans les suggestions
- [x] `Enter` pour assigner, `Escape` pour annuler
- [x] Afficher les assignees actuels avec possibilité de les retirer

### 4.3 Structures de données
- [x] Ajouter `assignees: Vec<String>` dans `IssueSummary` et `IssueDetail`
- [x] Afficher les assignees dans la vue liste et détail

### 4.4 Fichiers à modifier
- [x] `src/github.rs`: Nouvelles méthodes API
- [x] `src/tui.rs`: Nouvelle vue, logique d'auto-complete

### 4.5 Tests à écrire
- [x] Test `list_assignees()` avec wiremock
- [x] Test `assign_issue()` avec wiremock
- [x] Test `unassign_issue()` avec wiremock
- [x] Test fuzzy matching pour auto-complete

---

## 5. Intégration Claude Code

**Objectif:** Dispatcher des issues à Claude Code pour les traiter automatiquement.

### 5.1 Configuration projet

Ajouter `local_path` dans `assistant.json` pour chaque projet:

```json
{
  "projects": {
    "assistant": {
      "owner": "jean",
      "repo": "assistant",
      "labels": ["bug", "feature"],
      "local_path": "/Users/jean/Documents/developpement/assistant"
    }
  }
}
```

### 5.2 Module `src/agents/mod.rs`

#### 5.2.1 Structures de données
```rust
pub struct AgentSession {
    pub id: String,                 // UUID unique
    pub issue_number: u64,
    pub issue_title: String,
    pub project: String,
    pub started_at: DateTime<Utc>,
    pub status: AgentStatus,
    pub pid: u32,
    pub log_file: PathBuf,          // ~/.cache/assistant/agents/<id>.log
    pub worktree_path: PathBuf,     // ~/.cache/assistant/worktrees/<project>-<issue>/
    pub branch_name: String,        // issue-123
    pub stats: AgentStats,
    pub pr_url: Option<String>,     // URL de la PR si créée
}

pub struct AgentStats {
    pub lines_output: usize,        // Lignes dans le log Claude
    pub lines_added: usize,         // git diff --numstat
    pub lines_deleted: usize,
    pub files_changed: usize,
}

pub enum AgentStatus {
    Running,
    Completed { exit_code: i32 },
    Failed { error: String },
}
```

#### 5.2.2 Gestion des sessions
- [x] Fichier de sessions: `~/.cache/assistant/sessions.json`
- [x] `fn list_sessions() -> Vec<AgentSession>`
- [x] `fn get_session(id: &str) -> Option<AgentSession>`
- [x] `fn update_session_status(id: &str, status: AgentStatus)`
- [x] `fn cleanup_old_sessions(days: u32)` - supprime les vieilles sessions

### 5.3 Module `src/agents/claude.rs`

#### 5.3.1 Git Worktree pour isolation

Chaque issue est traitée dans un worktree isolé pour éviter les conflits:

```
~/.cache/assistant/worktrees/
├── <project>-<issue>/      # Un worktree par issue
│   └── ...                 # Copie complète du repo
```

**Flow de création:**
```bash
# 1. Créer une branche pour l'issue
git -C <local_path> branch issue-123 HEAD

# 2. Créer le worktree
git -C <local_path> worktree add ~/.cache/assistant/worktrees/project-123 issue-123
```

**Implémentation:**
- [x] `fn create_worktree(local_path, issue_number) -> Result<PathBuf>`
- [x] `fn remove_worktree(worktree_path) -> Result<()>`
- [x] Ajouter `worktree_path: PathBuf` dans `AgentSession`
- [x] Cleanup automatique des vieux worktrees (après X jours ou manuellement)

#### 5.3.2 Lancement de Claude Code
```rust
pub async fn dispatch_to_claude(
    issue: &IssueDetail,
    local_path: &Path,
) -> Result<AgentSession, AgentError>
```

- [x] Créer le worktree pour l'issue
- [x] Construire le prompt: `"Fix GitHub issue #{number}: {title}\n\n{body}"`
- [x] Commande: `claude -p "<prompt>"`
- [x] Working directory: le worktree (pas `local_path`)
- [x] Rediriger stdout/stderr vers le fichier de log
- [x] Lancer en background avec `std::process::Command`
- [x] Retourner immédiatement avec `AgentSession`

#### 5.3.3 Monitoring du processus
- [x] Thread/task de monitoring qui poll le processus
- [x] Compter les lignes de sortie en temps réel
- [x] **Git diff périodique** (toutes les 5s) pour stats de code:
  ```rust
  pub struct AgentStats {
      pub lines_output: usize,    // Lignes dans le log Claude
      pub lines_added: usize,     // git diff --numstat
      pub lines_deleted: usize,
      pub files_changed: usize,
  }
  ```
- [x] Détecter la fin du processus
- [x] Mettre à jour le statut dans `sessions.json`
- [x] Envoyer notification macOS à la fin

#### 5.3.4 Actions post-completion
- [x] Option "Create PR" → `gh pr create` depuis la branche du worktree
- [x] Option "View diff" → afficher le diff dans le TUI
- [x] Option "Cleanup" → supprimer le worktree et la branche
- [x] Option "Open in editor" → ouvrir le worktree dans VS Code/editor

### 5.4 Notifications macOS

- [x] Utiliser `osascript` ou crate `notify-rust`
- [x] À la fin: "Claude Code finished issue #123"
- [x] Clic sur notification → N/A (osascript doesn't support click handlers, use `/agents logs <id>` instead)

```rust
fn send_macos_notification(title: &str, message: &str) {
    Command::new("osascript")
        .args(["-e", &format!(
            "display notification \"{}\" with title \"{}\"",
            message, title
        )])
        .spawn()
        .ok();
}
```

### 5.5 Interface TUI

#### 5.5.1 Dispatch depuis la vue détail
- [x] Touche `d` pour dispatcher l'issue à Claude Code
- [x] Confirmation: "Dispatch #123 to Claude Code? (y/n)"
- [x] Message: "Agent started. View status with /agents"

#### 5.5.2 Multi-select dans la vue liste
- [x] Touche `Space` pour sélectionner/désélectionner une issue
- [x] Afficher un marqueur `[x]` devant les issues sélectionnées
- [x] Touche `d` avec sélection → dispatcher toutes les issues sélectionnées
- [x] Lancement en parallèle (une instance Claude Code par issue)

#### 5.5.3 Vue des sessions actives
- [x] Commande `/agents` ou touche `A` dans le TUI
- [x] Nouvelle vue `TuiView::AgentList`
- [x] Afficher: issue #, titre, statut, lignes output, durée
- [x] Navigation: `Enter` pour voir les logs, `K` pour kill un agent

```
┌──────────────────────────────────────────────────────────────────────┐
│ Agents (↑↓ navigate, Enter logs, p PR, o open, k kill, q back)       │
├──────────────────────────────────────────────────────────────────────┤
│ ▶ #123 Fix auth bug      Running    +47 -12   3 files   2m 30s       │
│   #124 Add dark mode     Completed  +203 -45  8 files   5m 12s  [PR] │
│   #125 Update deps       Failed     Error: timeout                   │
└──────────────────────────────────────────────────────────────────────┘
```

**Légende:**
- `+47 -12` : lignes ajoutées/supprimées (git diff)
- `3 files` : fichiers modifiés
- `[PR]` : PR créée depuis cette session

#### 5.5.4 Vue des logs
- [x] Nouvelle vue `TuiView::AgentLogs { session_id }`
- [x] Afficher le contenu du fichier de log
- [x] Scroll avec j/k
- [x] Refresh automatique si l'agent est encore en cours
- [x] `q` pour revenir à la liste des agents

### 5.6 Commande CLI `/agents`

- [x] `/agents` - Ouvre la vue des sessions actives
- [x] `/agents list` - Liste les sessions en mode texte
- [x] `/agents logs <id>` - Affiche les logs d'une session
- [x] `/agents kill <id>` - Termine un agent en cours
- [x] `/agents clean` - Supprime les vieilles sessions

### 5.7 Fichiers à créer/modifier

**Nouveaux fichiers:**
- [x] `src/agents/mod.rs`: Module principal, structures
- [x] `src/agents/claude.rs`: Intégration Claude Code
- [x] `src/agents/session.rs`: Gestion des sessions

**Fichiers à modifier:**
- [x] `src/config.rs`: Ajouter `local_path` à `ProjectConfig`
- [x] `src/tui.rs`: Nouvelles vues (AgentList, AgentLogs), multi-select
- [x] `src/main.rs`: Commande `/agents`
- [x] `src/lib.rs`: Exposer le module `agents`

### 5.8 Dépendances à ajouter
- [x] `uuid` pour les IDs de session
- [x] `chrono` pour les timestamps

### 5.9 Tests à écrire
- [x] Test `create_worktree()` - N/A (requires git repo, would need integration test)
- [x] Test `remove_worktree()` - N/A (requires git repo, would need integration test)
- [x] Test sérialisation/désérialisation `AgentSession` (JSON)
- [x] Test `list_sessions()` - via session_manager tests
- [x] Test `update_session_status()`
- [x] Test `update_session_stats()`
- [x] Test `cleanup_old_sessions()` - supprime les vieilles sessions
- [x] Test prompt generation (échappement des caractères spéciaux)

---

## 6. Intégrations futures (autres agents)

À implémenter après Claude Code, sur le même modèle:

### 6.1 Codex CLI
- Commande: `codex "<prompt>"`
- Même architecture que Claude Code

### 6.3 Architecture générique
Une fois les deux premiers agents implémentés, refactorer:
- Trait `Agent` avec `dispatch()`, `get_status()`, `kill()`
- Factory pour créer l'agent approprié
- Config optionnelle pour des agents custom

---

## Ordre d'implémentation

1. **Clôturer une issue** - ✅ Terminé
2. **Commande `/list` + recherche** - ✅ Terminé
3. **Infinite scrolling** - ✅ Terminé
4. **Attribution de personnes** - ✅ Terminé
5. **Intégration Claude Code** - ✅ Terminé:
   - 5a. Config `local_path` + structure sessions ✅
   - 5b. Lancement basique de Claude Code ✅
   - 5c. Monitoring + notifications ✅
   - 5d. TUI multi-select + vue agents ✅
   - 5e. Vue logs + commande `/agents` ✅

---

## Notes techniques

**Dépendances à ajouter:**
- `fuzzy-matcher` pour l'auto-complete (tâche 4)
- `uuid` pour les sessions (tâche 5)
- `chrono` pour les timestamps (tâche 5)

**Fichiers de cache:**
```
~/.cache/assistant/
├── sessions.json       # Liste des sessions actives/terminées
├── agents/
│   └── <uuid>.log      # Logs de chaque session
└── worktrees/
    └── <project>-<issue>/  # Git worktrees isolés
```

**Commandes de test:**
```bash
cargo build              # Compilation
cargo test               # Tests unitaires + intégration
cargo test -- --nocapture  # Avec output
cargo clippy             # Linting
cargo test <module>::tests  # Tests d'un module spécifique
```
