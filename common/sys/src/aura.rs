use common::{
    comp::{
        aura::{AuraChange, AuraKey, AuraKind, AuraTarget},
        buff,
        group::Group,
        Auras, BuffKind, Buffs, CharacterState, Health, Pos,
    },
    event::{EventBus, ServerEvent},
    resources::DeltaTime,
    uid::UidAllocator,
};
use specs::{saveload::MarkerAllocator, Entities, Join, Read, ReadStorage, System, WriteStorage};
use std::time::Duration;

pub struct Sys;
impl<'a> System<'a> for Sys {
    #[allow(clippy::type_complexity)]
    type SystemData = (
        Entities<'a>,
        Read<'a, DeltaTime>,
        ReadStorage<'a, Pos>,
        Read<'a, EventBus<ServerEvent>>,
        ReadStorage<'a, CharacterState>,
        WriteStorage<'a, Auras>,
        WriteStorage<'a, Buffs>,
        ReadStorage<'a, Health>,
        ReadStorage<'a, Group>,
        Read<'a, UidAllocator>,
    );

    fn run(
        &mut self,
        (
            entities,
            dt,
            positions,
            server_bus,
            character_states,
            mut auras,
            mut buffs,
            health,
            groups,
            uid_allocator,
        ): Self::SystemData,
    ) {
        let mut server_emitter = server_bus.emitter();

        auras.set_event_emission(false);

        // Iterate through all entities with an aura
        for (entity, pos, mut auras_comp) in (&entities, &positions, &mut auras).join() {
            let mut expired_auras = Vec::<AuraKey>::new();
            // Iterate through the auras attached to this entity
            for (key, aura) in auras_comp.auras.iter_mut() {
                // Tick the aura and subtract dt from it
                if let Some(remaining_time) = &mut aura.duration {
                    if let Some(new_duration) =
                        remaining_time.checked_sub(Duration::from_secs_f32(dt.0))
                    {
                        *remaining_time = new_duration;
                    } else {
                        *remaining_time = Duration::default();
                        expired_auras.push(key);
                    }
                }
                for (
                    target_entity,
                    target_pos,
                    target_character_state_maybe,
                    target_buffs,
                    health,
                ) in (
                    &entities,
                    &positions,
                    character_states.maybe(),
                    &mut buffs,
                    &health,
                )
                    .join()
                {
                    // Ensure entity is within the aura radius
                    if target_pos.0.distance_squared(pos.0) < aura.radius.powi(2) {
                        if let AuraTarget::GroupOf(uid) = aura.target {
                            let same_group = uid_allocator
                                .retrieve_entity_internal(uid.into())
                                .and_then(|e| groups.get(e))
                                .map_or(false, |owner_group| {
                                    Some(owner_group) == groups.get(target_entity)
                                });

                            if !same_group {
                                continue;
                            }
                        }

                        // TODO: When more aura kinds (besides Buff) are
                        // implemented, match on them here
                        match aura.aura_kind {
                            AuraKind::Buff {
                                kind,
                                data,
                                category,
                                source,
                            } => {
                                // Checks if the buff is not active so it isn't applied
                                // every tick, but rather only once it runs out
                                // TODO: Check for stronger buff of same kind so it can replace
                                // active buff.
                                if !target_buffs.contains(kind) {
                                    // Conditions for different buffs are in this match
                                    // statement
                                    let apply_buff = match kind {
                                        BuffKind::CampfireHeal => {
                                            matches!(
                                                target_character_state_maybe,
                                                Some(CharacterState::Sit)
                                            ) && health.current() < health.maximum()
                                        },
                                        // Add other specific buff conditions here
                                        _ => true,
                                    };
                                    if apply_buff {
                                        use buff::*;
                                        server_emitter.emit(ServerEvent::Buff {
                                            entity: target_entity,
                                            buff_change: BuffChange::Add(Buff::new(
                                                kind,
                                                data,
                                                vec![category],
                                                source,
                                            )),
                                        });
                                    }
                                }
                            },
                        }
                    }
                }
            }
            if !expired_auras.is_empty() {
                server_emitter.emit(ServerEvent::Aura {
                    entity,
                    aura_change: AuraChange::RemoveByKey(expired_auras),
                });
            }
        }
        auras.set_event_emission(true);
    }
}
