use futures::StreamExt;
use event_listener::{OnlineClient, PolkadotConfig};

pub async fn listen_event() {
	// Create a client to use:
	let api = OnlineClient::<PolkadotConfig>::new().await.unwrap();

	// Subscribe to any events that occur:
	let mut event_sub = api.events().subscribe().await.unwrap();

	loop {

		// Our subscription will see the events emitted as a result of this:
		while let Some(events) = event_sub.next().await {
			let events = events.unwrap();
			let block_hash = events.block_hash();
			log::info!("Dynamic event details: {}", block_hash);
			for event in events.iter() {
				let event = event.unwrap();
				let pallet = event.pallet_name();
				let variant = event.variant_name();
				log::info!("event details {}::{} ", pallet, variant);
			}
		}
	}
}
