#![allow(unused_assignments,unused_imports)]

use std::{collections::{HashMap, LinkedList}, sync::{Arc, atomic::Ordering}};

use actix_web::rt;

use super::{model::{Instance, InstanceTimeInfo, InstanceUpdateTag, UpdateInstanceType, ServiceKey}, api_model::QueryListResult, dal::service_do::ServiceDO};

#[derive(Debug,Clone,Default)]
pub struct ServiceMetadata {
    pub protect_threshold:f32,
}

#[derive(Debug,Clone,Default)]
pub struct Service {
    pub service_name:String,
    pub group_name:String,
    pub metadata:HashMap<String,String>,
    pub protect_threshold:f32,
    pub last_modified_millis:i64,
    //pub has_instance:bool,

    pub namespace_id:String,
    pub app_name:String,
    pub check_sum:String,
    pub(crate) instance_size:i64,
    //pub cluster_map:HashMap<String,Cluster>,

    pub(crate) instances: HashMap<String,Arc<Instance>>,
    pub(crate) timeinfos: LinkedList<InstanceTimeInfo>,

}

impl Service {
    pub(crate) fn recalculate_checksum(&mut self){
        self.check_sum="".to_owned();
    }

    /* 
    pub(crate) fn remove_instance(&mut self,cluster_name:&str,instance_id:&str) -> UpdateInstanceType {
        if let Some(cluster) = self.cluster_map.get_mut(cluster_name){
            cluster.remove_instance(instance_id);
            return UpdateInstanceType::Remove;
        }
        UpdateInstanceType::None
    }
    */

    pub(crate) fn update_instance(&mut self,mut instance:Instance,update_tag:Option<InstanceUpdateTag>) -> UpdateInstanceType {
        if instance.service_name=="service-consumer" {
            println!("service-consumer update_instance {:?}",&instance);
        }
        let key = instance.id.to_owned();
        let time_info = instance.get_time_info();
        //let mut update_mark = true;
        let mut rtype = UpdateInstanceType::None;
        let old_instance = self.instances.get(&key);
        if let Some(old_instance) = old_instance {
            rtype=UpdateInstanceType::UpdateValue;
            if let Some(update_tag) = update_tag {
                if(!update_tag.is_none()){
                    if !update_tag.enabled {
                        instance.enabled = old_instance.enabled.to_owned();
                    }
                    if !update_tag.ephemeral{
                        instance.ephemeral = old_instance.ephemeral.to_owned();
                    }
                    if !update_tag.weight{
                        instance.weight = old_instance.weight.to_owned();
                    }
                    if !update_tag.metadata{
                        instance.metadata = old_instance.metadata.to_owned();
                    }
                }
                else{
                    rtype=UpdateInstanceType::UpdateTime;
                }
            }
        }
        else{
            self.instance_size+=1;
            rtype=UpdateInstanceType::New;
        }
        let new_instance = Arc::new(instance);
        self.instances.insert(key, new_instance);
        self.update_timeinfos(time_info);
        /* 
        if update_mark {
            self.update_timeinfos(time_info);
        }
        */
        rtype
    }

    pub(crate) fn update_timeinfos(&mut self,time_info:InstanceTimeInfo) {
        for item in &mut self.timeinfos {
            if item.instance_id == time_info.instance_id {
                item.enable = false;
            }
        }
        self.timeinfos.push_back(time_info);
    }

    pub(crate) fn time_check(&mut self,healthy_time:i64,offline_time:i64) -> (Vec<String>,Vec<String>) {
        assert!(healthy_time>=offline_time);
        let mut i=0;
        let t= self.timeinfos.iter();
        let mut remove_list = vec![];
        let mut update_list = vec![];
        let mut remove_index = 0;
        for item in t {
            i+=1;
            if !item.enable { continue;}
            if item.time <= healthy_time {
                if item.time <= offline_time {
                    remove_list.push(item.instance_id.to_owned());
                    remove_index=i;
                }
                else{
                    update_list.push(item.instance_id.to_owned());
                }
            }
            else{
                break;
            }
        }
        self.timeinfos = self.timeinfos.split_off(remove_index);
        for item in &remove_list {
            self.remove_instance(&item);
        }
        for item in &update_list {
            self.update_instance_healthy_unvaild(&item);
        }
        (remove_list,update_list)
    }

    pub(crate) fn remove_instance(&mut self,instance_id:&str) -> UpdateInstanceType {
        if let Some(_)=self.instances.remove(instance_id){
            self.instance_size-=1;
            UpdateInstanceType::Remove
        }
        else{
            UpdateInstanceType::None
        }
    }

    pub(crate) fn update_instance_healthy_unvaild(&mut self,instance_id:&str) {
        if let Some(i) = self.instances.remove(instance_id) {
            let mut i = i.as_ref().clone();
            i.healthy=false;
            self.instances.insert(instance_id.to_owned(), Arc::new(i));
        }
    }

    pub(crate) fn get_instance(&self,instance_key:&str) -> Option<Arc<Instance>> {
        self.instances.get(instance_key).map_or(None, |i|Some(i.clone()))
    }

    pub(crate) fn get_all_instances(&self,only_healthy:bool) -> Vec<Arc<Instance>> {
        self.instances.values().filter(|x|
            x.enabled && (x.healthy || !only_healthy)).map(|x|x.clone()).collect::<Vec<_>>()
    }

    /*
    pub(crate) fn notify_listener(&mut self,cluster_name:&str,updateType:UpdateInstanceType) -> UpdateInstanceType {
        if match updateType {
            UpdateInstanceType::New =>{false}, 
            UpdateInstanceType::Remove =>{false}, 
            UpdateInstanceType::UpdateValue =>{false}, 
            _ => {true}
        }{
            return updateType;
        }
        updateType
    }
    */

    /* 
    pub(crate) fn get_instance(&self,cluster_name:&str,instance_key:&str) -> Option<Instance> {
        match self.cluster_map.get(cluster_name){
            Some(v) => {
                if let Some(i) = v.get_instance(instance_key) {
                    Some(i.clone())
                }
                else{ None}
            },
            None=> None,
        }
    }
    */
    
    pub(crate) fn get_instance_list(&self,cluster_names:Vec<String>,only_healthy:bool) -> Vec<Arc<Instance>> {
        self.get_all_instances(only_healthy)
        /* 
        let mut names = cluster_names;
        if names.len()==0 {
            for item in self.cluster_map.keys() {
                names.push(item.to_owned());
            }
        }
        for cluster_name in &names {
            if let Some(c) = self.cluster_map.get(cluster_name) {
                for item in c.get_all_instances(only_healthy) {
                    list.push(item.clone());
                }
            }
        }
        list
        */
    }

    /*
    pub(crate) fn get_instance_map(&self,cluster_names:Vec<String>,only_healthy:bool) -> HashMap<String,Vec<Instance>> {
        let mut map=HashMap::new();
        let mut names = cluster_names;
        if names.len()==0 {
            for item in self.cluster_map.keys() {
                names.push(item.to_owned());
            }
        }
        for cluster_name in &names {
            if let Some(c) = self.cluster_map.get(cluster_name) {
                let mut list = vec![];
                for item in c.get_all_instances(only_healthy) {
                    list.push(item.clone());
                }
                if list.len()>0 {
                    map.insert(cluster_name.to_owned(), list);
                }
            }
        }
        map
    }
     */

    /*
    pub(crate) fn get_instance_list_string(&self,cluster_names:Vec<String>,only_healthy:bool) -> String {
        let clusters = (&cluster_names).join(",");
        let list = self.get_instance_list(cluster_names, only_healthy);
        let key = ServiceKey::new(&self.namespace_id,&self.group_name,&self.service_name);
        QueryListResult::get_instance_list_string(clusters, &key, list)
    }
     */

     /* 
    pub(crate) fn time_check(&mut self,healthy_time:i64,offline_time:i64) -> (Vec<String>,Vec<String>) {
        let mut remove_list = vec![];
        let mut update_list = vec![];
        for item in self.cluster_map.values_mut() {
            let (mut rlist,mut ulist)=item.time_check(healthy_time, offline_time);
            remove_list.append(&mut rlist);
            update_list.append(&mut ulist);
        }
        (remove_list,update_list)
    }
     */

    pub fn get_service_key(&self) -> ServiceKey {
        ServiceKey::new(&self.namespace_id,&self.group_name,&self.service_name)
    }

    pub fn get_metadata(&self) -> ServiceMetadata {
        ServiceMetadata { protect_threshold: self.protect_threshold }
    }

    pub fn get_service_do(&self) -> ServiceDO {
        ServiceDO {
            namespace_id:Some(self.namespace_id.to_owned()),
            service_name:Some(self.service_name.to_owned()),
            group_name:Some(self.group_name.to_owned()),
            instance_size:Some(self.instance_size.to_owned()),
            ..Default::default()
        }
    }
}
