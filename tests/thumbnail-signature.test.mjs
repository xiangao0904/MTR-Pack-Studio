import test from 'node:test'
import assert from 'node:assert/strict'
import { carriageThumbnailSignature } from '../src/lib/thumbnail-signature.ts'

function fixture() {
  const layer={assetId:'shell',visible:true,flipTextureV:false,materialBindings:[],hiddenParts:[]}
  const carriage={name:'Carriage',length:20,width:3,bogie1Position:7,bogie2Position:-7,bodyModels:[layer],bogie1Models:[],bogie2Models:[]}
  const assets={shell:{previewHash:'glb-a',legacyUvCorrection:false}}
  return {layer,carriage,assets,signature:()=>carriageThumbnailSignature(carriage,assets)}
}

test('thumbnail stays cached for carriage metadata and preview-only changes',()=>{
  const f=fixture(),before=f.signature()
  f.carriage.name='Renamed';f.carriage.length=25;f.carriage.width=4;f.carriage.bogie1Position=9
  f.carriage.thumbnailHash='image-hash';f.carriage.thumbnailModelSignature='stored-signature'
  assert.equal(f.signature(),before)
})

test('thumbnail signature tracks model geometry, textures, UVs and visible placement',()=>{
  const f=fixture(),before=f.signature()
  f.layer.flipTextureV=true;assert.notEqual(f.signature(),before)
  f.layer.flipTextureV=false;f.layer.materialBindings.push({materialId:'paint',textureAssetId:'new-texture'});assert.notEqual(f.signature(),before)
  f.layer.materialBindings=[];f.assets.shell.previewHash='glb-b';assert.notEqual(f.signature(),before)
  f.assets.shell.previewHash='glb-a';f.layer.transform={translation:[0,1,0],rotation:[0,0,0],scale:[1,1,1]};assert.notEqual(f.signature(),before)
  f.layer.transform=undefined;f.carriage.bogie1Models.push(f.layer);assert.notEqual(f.signature(),before)
  const withBogie=f.signature();f.carriage.bogie1Position=8;assert.notEqual(f.signature(),withBogie)
})
