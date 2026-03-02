use tonic::{Request, Response, Status};
use tokio_stream::wrappers::ReceiverStream;
use tokio::sync::mpsc;

use crate::wal::wal_replication_server::WalReplication;
use crate::wal::{StreamRequest, WalEvent};

use std::sync::{Arc, Mutex};
use unicron_core::ClusterState;

#[derive(Clone)]
pub struct ReplicationService {
    pub cluster: Arc<Mutex<ClusterState>>,
}

#[tonic::async_trait]
impl WalReplication for ReplicationService {

    type StreamWalStream = ReceiverStream<Result<WalEvent, Status>>;

    async fn stream_wal(
        &self,
        _request: Request<StreamRequest>,
    ) -> Result<Response<Self::StreamWalStream>, Status> {

        let (_tx, rx) = mpsc::channel(8);

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}
