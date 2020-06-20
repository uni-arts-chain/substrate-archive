// Copyright 2017-2019 Parity Technologies (UK) Ltd.
// This file is part of substrate-archive.

// substrate-archive is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// substrate-archive is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with substrate-archive.  If not, see <http://www.gnu.org/licenses/>.

use crate::{
    actors::ArchiveContext,
    backend::{
        self, frontend::TArchiveClient, ApiAccess, ReadOnlyBackend, ReadOnlyDatabase,
        RuntimeApiCollection,
    },
    error::Error as ArchiveError,
    types::*,
};
use sc_chain_spec::ChainSpec;
use sc_client_api::backend as api_backend;
use sc_executor::NativeExecutionDispatch;
use sp_api::{ApiExt, ConstructRuntimeApi};
use sp_block_builder::BlockBuilder as BlockBuilderApi;
use sp_runtime::traits::{BlakeTwo256, Block as BlockT};
use std::{marker::PhantomData, sync::Arc};

pub struct Archive<Block: BlockT + Send + Sync, Spec: ChainSpec + Clone + 'static> {
    db_url: String,
    rpc_url: String,
    psql_url: Option<String>,
    cache_size: usize,
    spec: Spec,
    _marker: PhantomData<Block>,
}

pub struct ArchiveConfig {
    pub db_url: String,
    pub rpc_url: String,
    pub psql_url: Option<String>,
    pub cache_size: usize,
}

impl<Block, Spec> Archive<Block, Spec>
where
    Block: BlockT + Send + Sync,
    Spec: ChainSpec + Clone + 'static,
{
    /// Create a new instance of the Archive DB
    pub fn new(conf: ArchiveConfig, spec: Spec) -> Result<Self, ArchiveError> {
        Ok(Self {
            spec,
            db_url: conf.db_url,
            rpc_url: conf.rpc_url,
            psql_url: conf.psql_url,
            cache_size: conf.cache_size,
            _marker: PhantomData,
        })
    }

    /// returns an Api Client with the associated backend
    pub fn api_client<Runtime, Dispatch>(
        &self,
    ) -> Result<Arc<impl ApiAccess<Block, ReadOnlyBackend<Block>, Runtime>>, ArchiveError>
    where
        Runtime: ConstructRuntimeApi<Block, TArchiveClient<Block, Runtime, Dispatch>>
            + Send
            + Sync
            + 'static,
        Runtime::RuntimeApi: RuntimeApiCollection<
                Block,
                StateBackend = sc_client_api::StateBackendFor<ReadOnlyBackend<Block>, Block>,
            > + Send
            + Sync
            + 'static,
        Dispatch: NativeExecutionDispatch + 'static,
        <Runtime::RuntimeApi as sp_api::ApiExt<Block>>::StateBackend:
            sp_api::StateBackend<BlakeTwo256>,
    {
        let db = self.make_database()?;
        let backend =
            backend::runtime_api::<Block, Runtime, Dispatch>(db).map_err(ArchiveError::from)?;
        Ok(Arc::new(backend))
    }

    pub(crate) fn make_database(&self) -> Result<ReadOnlyDatabase, ArchiveError> {
        Ok(backend::util::open_database(
            self.db_url.as_str(),
            self.cache_size,
            self.spec.name(),
            self.spec.id(),
        )?)
    }

    pub(crate) fn make_backend<T>(&self) -> Result<ReadOnlyBackend<NotSignedBlock<T>>, ArchiveError>
    where
        T: Substrate + Send + Sync,
        <T as System>::BlockNumber: Into<u32>,
        <T as System>::Hash: From<primitive_types::H256>,
        <T as System>::Header: serde::de::DeserializeOwned,
    {
        let db = self.make_database()?;
        Ok(ReadOnlyBackend::new(db, true))
    }

    /// Returning context in which the archive is running
    pub fn run_with<T, Runtime, ClientApi>(
        &self,
        client_api: Arc<ClientApi>,
    ) -> Result<ArchiveContext<T>, ArchiveError>
    where
        T: Substrate + Send + Sync,
        Runtime: ConstructRuntimeApi<NotSignedBlock<T>, ClientApi> + Send + 'static,
        Runtime::RuntimeApi: BlockBuilderApi<NotSignedBlock<T>, Error = sp_blockchain::Error>
            + ApiExt<
                NotSignedBlock<T>,
                StateBackend = api_backend::StateBackendFor<
                    ReadOnlyBackend<NotSignedBlock<T>>,
                    NotSignedBlock<T>,
                >,
            >,
        ClientApi:
            ApiAccess<NotSignedBlock<T>, ReadOnlyBackend<NotSignedBlock<T>>, Runtime> + 'static,
        <T as System>::BlockNumber: Into<u32>,
        <T as System>::Hash: From<primitive_types::H256>,
        <T as System>::Header: serde::de::DeserializeOwned,
        <T as System>::BlockNumber: From<u32>,
    {
        let backend = Arc::new(self.make_backend::<T>()?);
        ArchiveContext::init(
            client_api,
            backend,
            self.rpc_url.clone(),
            self.psql_url.as_deref(),
        )
    }
}
