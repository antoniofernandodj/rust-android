use futures::Stream;
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::broadcast;

// ── Tipos públicos ────────────────────────────────────────────────────────────

/// Identificador opaco de uma tarefa enfileirada.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkId(String);

impl WorkId {
    fn new() -> Self {
        WorkId(uuid::Uuid::new_v4().to_string())
    }
}

impl std::fmt::Display for WorkId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Resultado retornado pelo closure de trabalho.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkResult {
    /// Trabalho concluído com sucesso.
    Success,
    /// Falha permanente; o WorkManager não vai tentar novamente.
    Failure,
    /// Falha transitória; o WorkManager vai reagendar com back-off.
    Retry,
}

/// Estado observável de uma tarefa.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkStatus {
    Enqueued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    /// Aguardando restrições (rede, bateria, etc.) serem satisfeitas.
    Blocked,
}

/// Restrições que devem ser satisfeitas antes de executar a tarefa.
#[derive(Debug, Clone, Default)]
pub struct Constraints {
    /// Requer conexão de rede (qualquer tipo).
    pub requires_network: bool,
    /// Requer rede não-medida (WiFi ou Ethernet).
    pub requires_unmetered_network: bool,
    /// Requer que o dispositivo esteja carregando.
    pub requires_charging: bool,
    /// Requer nível de bateria acima do mínimo.
    pub requires_battery_not_low: bool,
    /// Requer que o dispositivo esteja ocioso (idle).
    pub requires_device_idle: bool,
    /// Requer espaço de armazenamento disponível.
    pub requires_storage_not_low: bool,
}

impl Constraints {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn requires_network(mut self) -> Self {
        self.requires_network = true;
        self
    }

    pub fn requires_unmetered_network(mut self) -> Self {
        self.requires_unmetered_network = true;
        self
    }

    pub fn requires_charging(mut self) -> Self {
        self.requires_charging = true;
        self
    }

    pub fn requires_battery_not_low(mut self) -> Self {
        self.requires_battery_not_low = true;
        self
    }

    pub fn requires_device_idle(mut self) -> Self {
        self.requires_device_idle = true;
        self
    }
}

// ── Tipo de closure de trabalho ───────────────────────────────────────────────

type WorkFn = Box<dyn Fn() -> Pin<Box<dyn Future<Output = WorkResult> + Send>> + Send + Sync>;

// ── OneTimeWorkRequest ────────────────────────────────────────────────────────

/// Tarefa única agendada para rodar uma vez (imediatamente ou com delay).
pub struct OneTimeWorkRequest {
    pub(crate) id: WorkId,
    pub(crate) tag: Option<String>,
    pub(crate) constraints: Constraints,
    pub(crate) initial_delay: Option<Duration>,
    pub(crate) backoff: BackoffPolicy,
    pub(crate) work: WorkFn,
}

impl OneTimeWorkRequest {
    /// Cria um `OneTimeWorkRequest` a partir de um closure async.
    ///
    /// ```rust,ignore
    /// let req = OneTimeWorkRequest::new(|| async {
    ///     // Faz o trabalho
    ///     WorkResult::Success
    /// });
    /// ```
    pub fn new<F, Fut>(work: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = WorkResult> + Send + 'static,
    {
        OneTimeWorkRequest {
            id: WorkId::new(),
            tag: None,
            constraints: Constraints::default(),
            initial_delay: None,
            backoff: BackoffPolicy::default(),
            work: Box::new(move || Box::pin(work())),
        }
    }

    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    pub fn constraints(mut self, c: Constraints) -> Self {
        self.constraints = c;
        self
    }

    pub fn initial_delay(mut self, d: Duration) -> Self {
        self.initial_delay = Some(d);
        self
    }

    pub fn backoff(mut self, b: BackoffPolicy) -> Self {
        self.backoff = b;
        self
    }
}

// ── PeriodicWorkRequest ───────────────────────────────────────────────────────

/// Tarefa periódica que repete em intervalo fixo (mínimo 15 min no Android real).
pub struct PeriodicWorkRequest {
    pub(crate) id: WorkId,
    pub(crate) tag: Option<String>,
    pub(crate) interval: Duration,
    pub(crate) flex_interval: Option<Duration>,
    pub(crate) constraints: Constraints,
    pub(crate) backoff: BackoffPolicy,
    pub(crate) work: WorkFn,
}

impl PeriodicWorkRequest {
    /// Cria um `PeriodicWorkRequest`.
    ///
    /// `interval` é o período de repetição.  No Android real o mínimo é 15 min;
    /// no desktop stub é respeitado literalmente.
    ///
    /// ```rust,ignore
    /// let req = PeriodicWorkRequest::new(Duration::from_secs(900), || async {
    ///     WorkResult::Success
    /// });
    /// ```
    pub fn new<F, Fut>(interval: Duration, work: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = WorkResult> + Send + 'static,
    {
        PeriodicWorkRequest {
            id: WorkId::new(),
            tag: None,
            interval,
            flex_interval: None,
            constraints: Constraints::default(),
            backoff: BackoffPolicy::default(),
            work: Box::new(move || Box::pin(work())),
        }
    }

    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    pub fn constraints(mut self, c: Constraints) -> Self {
        self.constraints = c;
        self
    }

    /// Janela flexível dentro do intervalo em que a tarefa pode rodar.
    /// Útil para deixar o sistema otimizar o consumo de bateria.
    pub fn flex_interval(mut self, d: Duration) -> Self {
        self.flex_interval = Some(d);
        self
    }
}

// ── BackoffPolicy ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BackoffPolicy {
    pub strategy: BackoffStrategy,
    pub initial_delay: Duration,
}

#[derive(Debug, Clone)]
pub enum BackoffStrategy {
    Linear,
    Exponential,
}

impl Default for BackoffPolicy {
    fn default() -> Self {
        BackoffPolicy {
            strategy: BackoffStrategy::Exponential,
            initial_delay: Duration::from_secs(30),
        }
    }
}

// ── Registro interno (desktop stub) ──────────────────────────────────────────

#[derive(Clone)]
struct WorkEntry {
    id: WorkId,
    status_tx: broadcast::Sender<WorkStatus>,
}

static REGISTRY: Mutex<Option<Vec<WorkEntry>>> = Mutex::new(None);

fn registry() -> std::sync::MutexGuard<'static, Option<Vec<WorkEntry>>> {
    REGISTRY.lock().unwrap()
}

fn register(entry: WorkEntry) {
    let mut r = registry();
    r.get_or_insert_with(Vec::new).push(entry);
}

fn remove(id: &WorkId) -> Option<WorkEntry> {
    let mut r = registry();
    if let Some(list) = r.as_mut() {
        if let Some(pos) = list.iter().position(|e| &e.id == id) {
            return Some(list.remove(pos));
        }
    }
    None
}

fn find_sender(id: &WorkId) -> Option<broadcast::Sender<WorkStatus>> {
    let r = registry();
    r.as_ref()
        .and_then(|list| list.iter().find(|e| &e.id == id))
        .map(|e| e.status_tx.clone())
}

// ── WorkManager ───────────────────────────────────────────────────────────────

pub struct WorkManager;

impl WorkManager {
    /// Enfileira uma tarefa única.
    ///
    /// **Android**: delega ao `WorkManager` da Jetpack via JNI.  A tarefa
    /// persiste mesmo que o app feche e é executada quando as restrições
    /// forem satisfeitas.
    ///
    /// **Desktop**: executa a tarefa numa tokio task imediatamente (ignorando
    /// `initial_delay` e restrições, exceto em modo de teste).
    pub async fn enqueue(req: OneTimeWorkRequest) -> WorkId {
        let id = req.id.clone();
        let (tx, _) = broadcast::channel::<WorkStatus>(8);
        register(WorkEntry { id: id.clone(), status_tx: tx.clone() });

        let delay = req.initial_delay;
        let work = req.work;
        let tx2 = tx.clone();
        let id2 = id.clone();

        tokio::spawn(async move {
            if let Some(d) = delay {
                let _ = tx2.send(WorkStatus::Blocked);
                tokio::time::sleep(d).await;
            }
            let _ = tx2.send(WorkStatus::Running);
            let result = work().await;
            let final_status = match result {
                WorkResult::Success => WorkStatus::Succeeded,
                WorkResult::Failure | WorkResult::Retry => WorkStatus::Failed,
            };
            let _ = tx2.send(final_status);
            remove(&id2);
        });

        id
    }

    /// Enfileira uma tarefa periódica.
    ///
    /// **Android**: registra no `WorkManager` Jetpack com `setRepeatInterval`.
    ///
    /// **Desktop**: executa a tarefa em loop numa tokio task com o intervalo
    /// configurado.  A task continua até ser cancelada com [`WorkManager::cancel`].
    pub async fn enqueue_periodic(req: PeriodicWorkRequest) -> WorkId {
        let id = req.id.clone();
        let (tx, _) = broadcast::channel::<WorkStatus>(8);
        register(WorkEntry { id: id.clone(), status_tx: tx.clone() });

        let interval = req.interval;
        let work = req.work;
        let tx2 = tx.clone();
        let id2 = id.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;

                // Se a entrada foi removida (cancelada), encerra o loop.
                if find_sender(&id2).is_none() {
                    break;
                }

                let _ = tx2.send(WorkStatus::Running);
                let result = work().await;
                match result {
                    WorkResult::Success => { let _ = tx2.send(WorkStatus::Succeeded); }
                    WorkResult::Retry   => { let _ = tx2.send(WorkStatus::Enqueued); }
                    WorkResult::Failure => {
                        let _ = tx2.send(WorkStatus::Failed);
                        remove(&id2);
                        break;
                    }
                }
            }
        });

        id
    }

    /// Cancela uma tarefa pelo seu ID.
    ///
    /// **Android**: chama `WorkManager.cancelWorkById` via JNI.
    /// **Desktop**: remove do registro; o loop periódico encerrará na próxima iteração.
    pub async fn cancel(id: &WorkId) {
        if let Some(entry) = remove(id) {
            let _ = entry.status_tx.send(WorkStatus::Cancelled);
        }
    }

    /// Cancela todas as tarefas com a tag fornecida.
    ///
    /// **Android**: chama `WorkManager.cancelAllWorkByTag` via JNI.
    /// **Desktop**: remove todas as entradas com a tag (o `WorkEntry` não armazena
    /// tag por enquanto; esta versão cancela todas — refinamento futuro).
    pub async fn cancel_all() {
        let mut r = registry();
        if let Some(list) = r.as_mut() {
            for entry in list.drain(..) {
                let _ = entry.status_tx.send(WorkStatus::Cancelled);
            }
        }
    }

    /// Retorna um `Stream` com as mudanças de status de uma tarefa.
    ///
    /// **Android**: observa `LiveData<WorkInfo>` via JNI.
    /// **Desktop**: observa o channel broadcast interno.
    pub fn status(id: &WorkId) -> impl Stream<Item = WorkStatus> {
        let rx = find_sender(id).map(|tx| tx.subscribe());
        futures::stream::unfold(rx, |rx| async move {
            let mut rx = rx?;
            loop {
                match rx.recv().await {
                    Ok(s) => return Some((s, Some(rx))),
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return None,
                }
            }
        })
    }
}

// ── Alias de conveniência ─────────────────────────────────────────────────────

/// Agenda uma tarefa de sincronização de dados quando houver rede.
///
/// Açúcar sintático sobre `WorkManager::enqueue` com `Constraints::requires_network`.
pub async fn schedule_sync<F, Fut>(work: F) -> WorkId
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = WorkResult> + Send + 'static,
{
    WorkManager::enqueue(
        OneTimeWorkRequest::new(work)
            .constraints(Constraints::new().requires_network()),
    )
    .await
}
