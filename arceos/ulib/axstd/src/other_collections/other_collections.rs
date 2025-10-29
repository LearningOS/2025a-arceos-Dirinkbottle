use core::hash::{Hash, Hasher};
use core::mem;

///实现hashmap
///桶数组
/// 哈希函数
/// 冲突解决
/// 动态扩容

///桶:vec 桶存的是NODE 
/// NODE有key value next

///hash生成函数 axhal的random生成种子 这里hash函数使用异或和salt
/// 简单的链地址法解决冲突 axhal的random给每个hashmap生成不一样的salt，简单提高安全性。参照std的思路写出hashmap
extern crate axalloc;
extern crate axhal;
extern crate alloc;
use alloc::{boxed::Box,vec::Vec};
use axhal::misc::*;

struct HashNode<K,V>{
    key:K,
    value:V,
    next:Option<Box<HashNode<K,V>>>
}

impl<K,V> HashNode<K,V>{
    fn new(key:K, value:V)->Self{
        HashNode{
            key,
            value,
            next:None,
        }
    }
}

struct MyHasher{
    state:u64,
}

impl MyHasher{
    fn new()->Self{
        MyHasher{state:0}
    }
}

impl Hasher for MyHasher{
    fn finish(&self)->u64{
        self.state
    }
    
    fn write(&mut self, bytes:&[u8]){
        for &byte in bytes{
            // 简单的异或哈希
            self.state = self.state.wrapping_mul(31).wrapping_add(byte as u64);
        }
    }
}

pub struct HashMap<K,V>{
    salt:u64,
    capacity:usize,
    size:usize,
    bucket:Vec<Option<HashNode<K,V>>>
}

impl<K,V> HashMap<K,V>
where 
    K: Hash + PartialEq,
{
    //默认16的capacity
    pub fn new()->Self{
        let capacity:usize = 16;
        let mut buckets:Vec<Option<HashNode<K,V>>>=Vec::new();
        let salt:u64 = (random() as u64) ^ ((random() >> 64) as u64);
        for _ in 0..capacity{
            buckets.push(None);
        }
        HashMap{
            salt,
            capacity,
            size:0,
            bucket:buckets,
        }
    }
    
    // 计算哈希值并与salt异或
    fn hash(&self, key:&K)->usize{
        let mut hasher = MyHasher::new();
        key.hash(&mut hasher);
        let hash_value = hasher.finish() ^ self.salt; // 与salt异或
        (hash_value as usize) % self.capacity
    }
    
    // 插入键值对
    pub fn insert(&mut self, key:K, value:V)->Option<V>{
        // 如果负载因子超过0.75，进行扩容
        if self.size >= self.capacity * 3 / 4{
            self.resize();
        }
        
        let index = self.hash(&key);
        
        // 检查桶中是否已存在该key
        if let Some(ref mut node) = self.bucket[index]{
            // 在链表中查找
            let mut current = node;
            loop{
                if current.key == key{
                    // 找到了，更新value并返回旧值
                    let old_value = mem::replace(&mut current.value, value);
                    return Some(old_value);
                }
                
                if current.next.is_none(){
                    break;
                }
                current = current.next.as_mut().unwrap();
            }
            
            // 没找到，在链表末尾插入
            current.next = Some(Box::new(HashNode::new(key, value)));
            self.size += 1;
            None
        }else{
            // 桶为空，直接插入
            self.bucket[index] = Some(HashNode::new(key, value));
            self.size += 1;
            None
        }
    }
    
    // 查找键对应的值
    pub fn get(&self, key:&K)->Option<&V>{
        let index = self.hash(key);
        
        if let Some(ref node) = self.bucket[index]{
            let mut current = node;
            loop{
                if current.key == *key{
                    return Some(&current.value);
                }
                
                if let Some(ref next) = current.next{
                    current = next;
                }else{
                    break;
                }
            }
        }
        None
    }
    
    // 扩容
    fn resize(&mut self){
        let new_capacity = self.capacity * 2;
        let mut new_buckets:Vec<Option<HashNode<K,V>>> = Vec::new();
        for _ in 0..new_capacity{
            new_buckets.push(None);
        }
        
        let old_buckets = mem::replace(&mut self.bucket, new_buckets);
        self.capacity = new_capacity;
        self.size = 0;
        
        // 重新插入所有元素
        for mut bucket in old_buckets{
            while let Some(mut node) = bucket{
                bucket = node.next.take().map(|boxed| *boxed);
                self.insert(node.key, node.value);
            }
        }
    }
    
    // 返回迭代器
    pub fn iter(&self)->HashMapIter<K,V>{
        HashMapIter{
            map:self,
            bucket_index:0,
            current_node:None,
        }
    }
    
    // 返回大小
    pub fn len(&self)->usize{
        self.size
    }
    
    // 判断是否为空
    pub fn is_empty(&self)->bool{
        self.size == 0
    }
}

// 迭代器结构
pub struct HashMapIter<'a, K, V>{
    map:&'a HashMap<K,V>,
    bucket_index:usize,
    current_node:Option<&'a HashNode<K,V>>,
}

impl<'a, K, V> Iterator for HashMapIter<'a, K, V>
where
    K: Hash + PartialEq,
{
    type Item = (&'a K, &'a V);
    
    fn next(&mut self)->Option<Self::Item>{
        loop{
            // 如果当前有节点，返回它
            if let Some(node) = self.current_node{
                let result = (&node.key, &node.value);
                // 移动到下一个节点
                self.current_node = node.next.as_ref().map(|boxed| &**boxed);
                return Some(result);
            }
            
            // 当前没有节点，查找下一个非空桶
            if self.bucket_index >= self.map.capacity{
                return None;
            }
            
            if let Some(ref node) = self.map.bucket[self.bucket_index]{
                self.current_node = Some(node);
            }
            
            self.bucket_index += 1;
        }
    }
}
