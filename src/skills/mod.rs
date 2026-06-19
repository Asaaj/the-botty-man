pub mod schedule;

use std::{collections::HashMap, fmt::Debug};

use anyhow::{Result, anyhow};
use schedule::Schedule;
use serenity::{
    all::{
        CommandInteraction, ComponentInteraction, Context, CreateCommand, Interaction,
        ModalInteraction,
    },
    async_trait,
};

#[async_trait]
pub trait Skill: Send + Sync + Debug {
    fn name(&self) -> String;

    fn register(&self, ctx: &Context) -> CreateCommand;

    async fn command(&self, _ctx: &Context, _command: &CommandInteraction) -> Result<()> {
        Err(anyhow!("{} does not support commands", self.name()))
    }

    async fn component(&self, _ctx: &Context, _component: &ComponentInteraction) -> Result<()> {
        Err(anyhow!("{} does not support components", self.name()))
    }

    async fn modal(&self, _ctx: &Context, _modal: &ModalInteraction) -> Result<()> {
        Err(anyhow!("{} does not support modals", self.name()))
    }
}

#[derive(Debug)]
pub struct SkillRegistry {
    skills: HashMap<String, Box<dyn Skill>>,
}
impl SkillRegistry {
    pub async fn new(ctx: &Context) -> Self {
        let skills: Vec<Box<dyn Skill>> = vec![Box::new(Schedule::new(ctx).await)];
        let skills = skills.into_iter().map(|s| (s.name(), s)).collect();
        Self { skills }
    }

    pub fn all_commands(&self, ctx: &Context) -> Vec<CreateCommand> {
        self.skills.values().map(|s| s.register(ctx)).collect()
    }

    pub async fn dispatch(&self, ctx: &Context, interaction: &Interaction) -> Result<()> {
        match interaction {
            Interaction::Command(command) => {
                if let Some(skill) = self.skills.get(&command.data.name) {
                    skill.command(ctx, command).await
                } else {
                    Err(anyhow!("{} is not a registered command", command.data.name))
                }
            }
            Interaction::Component(component) => {
                if let Some(skill) = self.skill_for_id(&component.data.custom_id) {
                    skill.component(ctx, component).await
                } else {
                    Err(anyhow!(
                        "{} is not a registered component",
                        component.data.custom_id
                    ))
                }
            }
            Interaction::Modal(modal) => {
                if let Some(skill) = self.skill_for_id(&modal.data.custom_id) {
                    skill.modal(ctx, modal).await
                } else {
                    Err(anyhow!(
                        "{} is not a registered modal",
                        modal.data.custom_id
                    ))
                }
            }
            _ => Ok(()),
        }
    }

    // Routes component and modal IDs to skills by prefix: "schedule_*" → schedule skill.
    fn skill_for_id(&self, id: &str) -> Option<&Box<dyn Skill>> {
        self.skills
            .values()
            .find(|s| id.starts_with(s.name().as_str()))
    }
}
