use crate::{
    action::ActionWrapper,
    agent::chain_header,
    context::Context,
    network::{actions::ActionResponse, state::NetworkState, EntryWithHeader},
};
use holochain_core_types::{
    cas::content::{Address, AddressableContent},
    entry::{Entry, SerializedEntry},
    error::HolochainError,
};
use holochain_net_connection::{
    net_connection::NetConnection,
    protocol_wrapper::{DhtData, ProtocolWrapper},
};
use std::{convert::TryInto, sync::Arc};

fn entry_from_cas(address: &Address, context: &Arc<Context>) -> Result<Entry, HolochainError> {
    let json = context
        .file_storage
        .read()?
        .fetch(address)?
        .ok_or("Entry not found".to_string())?;
    let s: SerializedEntry = json.try_into()?;
    Ok(s.into())
}

pub fn reduce_publish(
    context: Arc<Context>,
    network_state: &mut NetworkState,
    action_wrapper: &ActionWrapper,
) {
    if network_state.network.is_none()
        || network_state.dna_hash.is_none()
        || network_state.agent_id.is_none()
    {
        return;
    }

    let action = action_wrapper.action();
    let address = unwrap_to!(action => crate::action::Action::Publish);

    let result = entry_from_cas(address, &context);
    if result.is_err() {
        return;
    };

    let (entry, maybe_header) = result
        .map(|entry| {
            let header = chain_header(&entry, &context);
            (entry, header)
        })
        .unwrap();

    if maybe_header.is_none() {
        // We don't have the entry in our source chain?!
        // Don't publish
        return;
    }

    let entry_with_header = EntryWithHeader {
        entry: entry.serialize(),
        header: maybe_header.unwrap(),
    };

    //let header = maybe_header.unwrap();
    let data = DhtData {
        msg_id: "?".to_string(),
        dna_hash: network_state.dna_hash.clone().unwrap(),
        agent_id: network_state.agent_id.clone().unwrap(),
        address: entry.address().to_string(),
        content: serde_json::from_str(&serde_json::to_string(&entry_with_header).unwrap()).unwrap(),
    };

    let response = match network_state.network {
        None => unreachable!(),
        Some(ref network) => network
            .lock()
            .unwrap()
            .send(ProtocolWrapper::PublishDht(data).into()),
    };

    network_state.actions.insert(
        action_wrapper.clone(),
        ActionResponse::Publish(match response {
            Ok(_) => Ok(entry.address().to_owned()),
            Err(e) => Err(HolochainError::ErrorGeneric(e.to_string())),
        }),
    );
}

#[cfg(test)]
mod tests {

    use crate::{
        action::{Action, ActionWrapper},
        instance::tests::test_context,
        state::test_store,
    };
    use holochain_core_types::{cas::content::AddressableContent, entry::test_entry};

    #[test]
    pub fn reduce_publish_test() {
        let context = test_context("alice");
        let store = test_store(context.clone());

        let entry = test_entry();
        let action_wrapper = ActionWrapper::new(Action::Publish(entry.address()));

        store.reduce(context.clone(), action_wrapper);
    }

}
