//! Resource production/consumption system.
//!
//! Producers run before consumers, both in stable `BTreeMap` id order.
//! That makes same-tick chains deterministic: stock produced at tick N may
//! be consumed by a consumer that also fires at tick N.

use crate::components::{ConsumerId, ProducerId, ResourceId};
use crate::world::World;

pub type ProductionEntry = (ProducerId, ResourceId, u64);
pub type ConsumptionEntry = (ConsumerId, ResourceId, u64);

#[derive(Debug, Default)]
pub struct ProductionScratch {
    produced: Vec<ProductionEntry>,
    consumed: Vec<ConsumptionEntry>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ProductionStats {
    pub produced: u64,
    pub consumed: u64,
}

pub fn run(world: &mut World, scratch: &mut ProductionScratch) -> ProductionStats {
    scratch.produced.clear();
    scratch.consumed.clear();

    for (pid, producer) in &world.producers {
        if fires(world.tick, producer.interval_ticks) {
            scratch
                .produced
                .push((*pid, producer.resource, producer.amount));
        }
    }

    let mut stats = ProductionStats::default();
    for &(_, resource, amount) in &scratch.produced {
        let slot = world.inventory.entry(resource).or_insert(0);
        *slot = slot.saturating_add(amount);
        stats.produced = stats.produced.saturating_add(amount);
    }

    for (cid, consumer) in &world.consumers {
        if !fires(world.tick, consumer.interval_ticks) {
            continue;
        }
        let available = world
            .inventory
            .get(&consumer.resource)
            .copied()
            .unwrap_or(0);
        if available >= consumer.amount {
            scratch
                .consumed
                .push((*cid, consumer.resource, consumer.amount));
        }
    }

    for &(_, resource, amount) in &scratch.consumed {
        if let Some(slot) = world.inventory.get_mut(&resource) {
            *slot = slot.saturating_sub(amount);
            stats.consumed = stats.consumed.saturating_add(amount);
        }
    }

    stats
}

fn fires(tick: u64, interval_ticks: u32) -> bool {
    interval_ticks > 0 && tick % u64::from(interval_ticks) == 0
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::components::{Consumer, Producer, Resource, ResourceId};
    use crate::tick::TickRunner;

    fn world() -> World {
        let mut world = World::new(0);
        let ore = ResourceId(0);
        world.resources.insert(
            ore,
            Resource {
                id: ore,
                name: "ore".to_string(),
                color: 1,
            },
        );
        world.inventory.insert(ore, 0);
        world.producers.insert(
            ProducerId(0),
            Producer {
                id: ProducerId(0),
                resource: ore,
                amount: 3,
                interval_ticks: 2,
            },
        );
        world.consumers.insert(
            ConsumerId(0),
            Consumer {
                id: ConsumerId(0),
                resource: ore,
                amount: 2,
                interval_ticks: 2,
            },
        );
        world
    }

    #[test]
    fn producer_then_consumer_same_tick_is_deterministic() {
        let mut world = world();
        world.tick = 2;
        let mut scratch = ProductionScratch::default();

        let stats = run(&mut world, &mut scratch);

        assert_eq!(stats.produced, 3);
        assert_eq!(stats.consumed, 2);
        assert_eq!(world.inventory.get(&ResourceId(0)), Some(&1));
    }

    #[test]
    fn tick_runner_advances_inventory_on_intervals() {
        let mut world = world();
        let mut runner = TickRunner::new();

        runner.tick_once(&mut world);
        assert_eq!(world.inventory.get(&ResourceId(0)), Some(&0));

        runner.tick_once(&mut world);
        assert_eq!(world.inventory.get(&ResourceId(0)), Some(&1));

        runner.tick_once(&mut world);
        assert_eq!(world.inventory.get(&ResourceId(0)), Some(&1));
    }
}
