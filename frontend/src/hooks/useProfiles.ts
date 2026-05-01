import { useState, useEffect, useCallback } from 'react';

export interface ProfileInfo {
  id: string;
  name: string;
  color: string;
  avatarInitial: string;
  avatarImage?: string; // Base64 data URL for custom avatar
}

export interface ProfilesState {
  currentProfileId: string;
  defaultProfileId: string;
  profiles: ProfileInfo[];
}

export function useProfiles() {
  const [profiles, setProfiles] = useState<ProfileInfo[]>([]);
  const [currentProfileId, setCurrentProfileId] = useState<string>('Default');
  const [defaultProfileId, setDefaultProfileId] = useState<string>('Default');
  const [loading, setLoading] = useState(true);

  // Fetch profiles on mount
  useEffect(() => {
    fetchProfiles();

    // Set up listener for profile results
    const handleProfilesResult = (data: ProfilesState) => {
      console.log('Profiles received:', data);
      setProfiles(data.profiles || []);
      setCurrentProfileId(data.currentProfileId || 'Default');
      setDefaultProfileId(data.defaultProfileId || 'Default');
      setLoading(false);
    };

    // @ts-ignore - window callback
    window.onProfilesResult = handleProfilesResult;

    return () => {
      // @ts-ignore
      delete window.onProfilesResult;
    };
  }, []);

  const fetchProfiles = useCallback(() => {
    setLoading(true);
    window.cefMessage?.send('profiles_get_all', []);
  }, []);

  const createProfile = useCallback((name: string, color: string, avatarImage?: string) => {
    const args = avatarImage ? [name, color, avatarImage] : [name, color];
    window.cefMessage?.send('profiles_create', args);
  }, []);

  const renameProfile = useCallback((id: string, newName: string) => {
    window.cefMessage?.send('profiles_rename', [id, newName]);
    // Optimistic update
    setProfiles(prev => prev.map(p =>
      p.id === id ? { ...p, name: newName, avatarInitial: newName[0]?.toUpperCase() || '?' } : p
    ));
  }, []);

  const deleteProfile = useCallback((id: string) => {
    window.cefMessage?.send('profiles_delete', [id]);
    // Optimistic update
    setProfiles(prev => prev.filter(p => p.id !== id));
  }, []);

  const switchProfile = useCallback((id: string) => {
    console.log('Switching to profile:', id);
    window.cefMessage?.send('profiles_switch', [id]);
    // New window will open, this one can stay or close
  }, []);

  const setProfileColor = useCallback((id: string, color: string) => {
    window.cefMessage?.send('profiles_set_color', [id, color]);
    // Optimistic update
    setProfiles(prev => prev.map(p => p.id === id ? { ...p, color } : p));
  }, []);

  const setProfileAvatar = useCallback((id: string, avatarImage: string) => {
    window.cefMessage?.send('profiles_set_avatar', [id, avatarImage]);
    // Optimistic update
    setProfiles(prev => prev.map(p => p.id === id ? { ...p, avatarImage } : p));
  }, []);

  const setDefaultProfile = useCallback((id: string) => {
    window.cefMessage?.send('profiles_set_default', [id]);
    setDefaultProfileId(id);
  }, []);

  const currentProfile = profiles.find(p => p.id === currentProfileId) || profiles[0];

  return {
    profiles,
    currentProfile,
    currentProfileId,
    defaultProfileId,
    loading,
    fetchProfiles,
    createProfile,
    renameProfile,
    deleteProfile,
    switchProfile,
    setProfileColor,
    setProfileAvatar,
    setDefaultProfile,
  };
}
