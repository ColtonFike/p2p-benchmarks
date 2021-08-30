use async_std::{io, task};
use futures::{future, prelude::*};
use libp2p::{
    floodsub::{self, Floodsub, FloodsubEvent},
    identity,
    mdns::{Mdns, MdnsConfig, MdnsEvent},
    swarm::{NetworkBehaviourEventProcess, SwarmEvent}, NetworkBehaviour, PeerId, Swarm,
};
use std::{
    error::Error,
    task::{Context, Poll},
    time::Instant,
};

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!("Local peer id: {:?}", local_peer_id);

    let transport = libp2p::development_transport(local_key).await?;

    let floodsub_topic = floodsub::Topic::new("one-to-all");

    let args: Vec<String> = std::env::args().collect();
    let leader: bool = if args[1].parse::<i32>()? == 1 { true } else { false };
    let nodes: i32 = args[2].parse::<i32>()?;
    println!("{}",nodes);
    const SIZE: usize = 500;

    #[derive(NetworkBehaviour)]
    struct MyBehaviour {
        floodsub: Floodsub,
        mdns: Mdns,
        #[behaviour(ignore)]
        #[allow(dead_code)]
        ignored_member: bool,
        #[behaviour(ignore)]
        now: Instant,
        #[behaviour(ignore)]
        count: i32,
        #[behaviour(ignore)]
        payload: [u8; SIZE],
        #[behaviour(ignore)]
        leader: bool,
        #[behaviour(ignore)]
        nodes: i32,
        #[behaviour(ignore)]
        sum: u128,
        #[behaviour(ignore)]
        iterations: i32,
    }

    impl NetworkBehaviourEventProcess<FloodsubEvent> for MyBehaviour {
        fn inject_event(&mut self, message: FloodsubEvent) {
            if let FloodsubEvent::Message(message) = message {
                //println!("Received: from {:?}", message.source);
                self.count += 1;

                let ota = floodsub::Topic::new("one-to-all");
                let ato = floodsub::Topic::new("all-to-one");
                if !self.leader && message.topics[0] == ota {
                    self.floodsub.publish_any(ato, self.payload);
                }
                if self.leader && self.count >= self.nodes {
                    self.iterations += 1;
                    self.sum += self.now.elapsed().as_micros();
                    println!("Average time: {}", self.sum as f64 / self.iterations as f64);
                    self.count = 0;
                    self.now = Instant::now();
                    self.floodsub.publish(ota, self.payload);
                }
            }
        }
    }

    impl NetworkBehaviourEventProcess<MdnsEvent> for MyBehaviour {
        fn inject_event(&mut self, event: MdnsEvent) {
            match event {
                MdnsEvent::Discovered(list) => {
                    for (peer, _) in list {
                        self.floodsub.add_node_to_partial_view(peer);
                    }
                }
                MdnsEvent::Expired(list) => {
                    for (peer, _) in list {
                        if !self.mdns.has_node(&peer) {
                            self.floodsub.remove_node_from_partial_view(&peer);
                        }
                    }
                }
            }
        }
    }

    let mut swarm = {
        let mdns = task::block_on(Mdns::new(MdnsConfig::default()))?;
        let mut behaviour = MyBehaviour {
            floodsub: Floodsub::new(local_peer_id),
            mdns,
            ignored_member: false,
            now: Instant::now(),
            count: 0,
            payload: [0; SIZE],
            leader: leader,
            nodes: nodes,
            sum: 0,
            iterations: 0,
        };
        behaviour.floodsub.subscribe(floodsub_topic.clone());

        if behaviour.leader {
            behaviour.floodsub.subscribe(floodsub::Topic::new("all-to-one"));
        }
        Swarm::new(transport, behaviour, local_peer_id)
    };

    let mut stdin = io::BufReader::new(io::stdin()).lines();

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    task::block_on(future::poll_fn(move |cx: &mut Context<'_>| {
        loop {
            match stdin.try_poll_next_unpin(cx)? {
                Poll::Ready(Some(line)) => {
                    swarm
                    .behaviour_mut()
                    .floodsub
                    .publish(floodsub_topic.clone(), line.as_bytes());
                    swarm.behaviour_mut().now = Instant::now();
                },
                Poll::Ready(None) => break,
                Poll::Pending => break,
            }
        }
        loop {
            match swarm.poll_next_unpin(cx) {
                Poll::Ready(Some(event)) => {
                    if let SwarmEvent::NewListenAddr { address, .. } = event {
                        println!("Listening on {:?}", address)
                    }
                }
                Poll::Ready(None) => return Poll::Ready(Ok(())),
                Poll::Pending => break,
            }
        }
        Poll::Pending
    }))
}
